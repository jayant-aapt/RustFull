import psutil
import json
import os
import time
import logging
from datetime import datetime
from collections import defaultdict
from sqlalchemy import create_engine, MetaData, Table
from sqlalchemy.orm import sessionmaker
import wmi
import re
import pythoncom
import subprocess

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

TABLE_UUID_MAP = {
    "memory_monitoring": "memory",
    "cpu_monitoring": "cpu",
    "disk_monitoring": "storage",
    "partition_monitoring": "partition",
    "network_monitoring": "port",
    "device": "device",
}

class Monitoring:
    def _init_wmi(self):
        try:
            pythoncom.CoUninitialize() 
        except pythoncom.com_error:
            pass
        try:
            pythoncom.CoInitialize()
            self.wmi_obj = wmi.WMI()
        except Exception as e:
            logging.error(f"Failed to initialize WMI: {e}")

    def __init__(self):
        self._init_wmi()
        db_path = os.path.abspath(r"C:\Users\Administrator\Desktop\Rust_project\RustFull\models_database\models_database.sqlite")
        self.engine = create_engine(f"sqlite:///{db_path}")
        Session = sessionmaker(bind=self.engine)
        self.session = Session()
        self.metadata = MetaData()
        self.metadata.reflect(bind=self.engine)
        self.db_path = db_path
        self.last_mod_time = os.path.getmtime(self.db_path)
        self.uuid_cache = {}
        self.hardware_identifiers = {
            'memory_serial_number': None,
            'cpu_processor_id': None,
            'disk_serial_number': None,
            'partition_volume_serials': {},
        }
        self.cache_hardware_identifiers()

    def __del__(self):
        try:
            pythoncom.CoUninitialize()
        except:
            pass

    def cache_hardware_identifiers(self):
        if not self.wmi_obj:
            logging.warning("Skipping hardware identifier caching because WMI is not available.")
            return

        try:
            system = self.wmi_obj.Win32_ComputerSystem()[0]
            model=system.Model.strip() if system.Model else "Unknown"
            self.hardware_identifiers['device_model'] = model
        except Exception as e:
            logging.error("Error retrieving device model: %s", e)

        try:
            for mem in self.wmi_obj.Win32_PhysicalMemory():
                self.hardware_identifiers['memory_make'] =  mem.Manufacturer.strip()
                break
        except Exception as e:
            logging.error("Error retrieving memory serial number: %s", e)

        try:
            self.hardware_identifiers['cpu_model'] = self.wmi_obj.Win32_Processor()[0].Name.strip()
        except Exception as e:
            logging.error("Error retrieving CPU model: %s", e)

        try:
            command = r'powershell -Command "Get-Disk | Select-Object Number, FriendlyName, UniqueId | ConvertTo-Json"'
            result = subprocess.check_output(command, shell=True, universal_newlines=True)
            ps_disks = json.loads(result)
            if isinstance(ps_disks, dict):
                ps_disks = [ps_disks]

            disk_uuid_map = {
                d["Number"]: d["UniqueId"].strip().replace(" ", "") if d.get("UniqueId") else "Unknown"
                for d in ps_disks
            }

            valid_uuids = [uuid for uuid in disk_uuid_map.values() if uuid != "Unknown"]
            if valid_uuids:
                self.hardware_identifiers['disk_serial_numbers'] = valid_uuids
        except Exception as e:
            logging.error("Error retrieving disk serial numbers: %s", e)

        try:
            if self.wmi_obj:
                for disk in self.wmi_obj.Win32_LogicalDisk():
                    if disk.DriveType == 3 and disk.VolumeSerialNumber and disk.DeviceID:
                        self.hardware_identifiers['partition_volume_serials'][disk.DeviceID.upper()] = disk.VolumeSerialNumber
        except Exception as e:
            logging.error("Error retrieving partition volume serial numbers: %s", e)

    def has_db_changed(self):
        mod_time = os.path.getmtime(self.db_path)
        if mod_time != self.last_mod_time:
            self.last_mod_time = mod_time
            self.uuid_cache.clear()
            self._init_wmi()
            self.cache_hardware_identifiers()
            return True
        return False

    def get_uuid_by_name(self, logical_table_name, name_field, name_value):
        if not name_value:
            logging.error(f"Missing name_value for table: {logical_table_name}, field: {name_field}")
            return ("unknown", "unknown") if logical_table_name == "partition_monitoring" or logical_table_name=="network_monitoring" else "unknown"

        if self.has_db_changed():
            logging.info("Database modified — clearing cache.")

        cache_key = (logical_table_name, name_value)
        if cache_key in self.uuid_cache:
            return self.uuid_cache[cache_key]

        table_name = TABLE_UUID_MAP.get(logical_table_name)
        if not table_name:
            return ("unknown", "unknown") if logical_table_name == "partition_monitoring" or logical_table_name=="network_monitoring" else "unknown"

        try:
            table = Table(table_name, self.metadata, autoload_with=self.engine)

            if logical_table_name == "partition_monitoring":
                result = self.session.query(table.c.uuid, table.c.storage_uuid) \
                                    .filter(getattr(table.c, name_field) == name_value) \
                                    .first()

                uuid, storage_uuid = (result.uuid, result.storage_uuid) if result and result.uuid and result.storage_uuid else ("unknown", "unknown")

                self.uuid_cache[cache_key] = (uuid, storage_uuid)
                return uuid, storage_uuid
            elif logical_table_name == "network_monitoring":
                result = self.session.query(table.c.uuid, table.c.nic_uuid) \
                                    .filter(getattr(table.c, name_field) == name_value) \
                                    .first()

                uuid, nic_uuid = (result.uuid, result.nic_uuid) if result and result.uuid and result.nic_uuid else ("unknown", "unknown")

                self.uuid_cache[cache_key] = (uuid, nic_uuid)
                return uuid, nic_uuid
            else:
                result = self.session.query(table.c.uuid)\
                                    .filter(getattr(table.c, name_field) == name_value).first()
                uuid = result[0] if result else "unknown"
                self.uuid_cache[cache_key] = uuid
                return uuid
        except Exception as e:
            logging.error(f"Error fetching UUID for {name_value} in {logical_table_name}: {e}")
            return ("unknown", "unknown") if logical_table_name == "partition_monitoring" or logical_table_name=="network_monitoring" else "unknown"


    def get_monitoring_checkpoint(self):
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        return {
            "device_uuid": self.get_uuid_by_name("device", "model", self.hardware_identifiers.get('device_model', 'Unknown')),
            "event_type": "MON_DATA",
            "description": "monitoring data",
            "date": timestamp.split()[0],
            "time": timestamp.split()[1],
            "memory_monitoring": self.get_memory_info(),
            "cpu_monitoring": self.get_cpu_info(),
            "disk_monitoring": self.get_disk_info(),
            "partition_monitoring": self.partition_monitoring(),
            "network_monitoring": self.network_monitoring()
        }

    def get_memory_info(self):
        memory_info = psutil.virtual_memory()
        uuid = self.get_uuid_by_name("memory_monitoring", "make", self.hardware_identifiers.get('memory_make', 'Virtual'))
        return {
            "memory_uuid": uuid,
            "memory_used": round(memory_info.used),
            "memory_available": round(memory_info.available),
            "total_memory": round(memory_info.total)
        }

    def get_cpu_info(self):
        cpu_stats = psutil.cpu_stats()
        logical_usages = psutil.cpu_percent(percpu=True)
        physical_core_map = defaultdict(list)
        core_count = psutil.cpu_count(logical=False)
        logical_count = psutil.cpu_count(logical=True)

        for i in range(logical_count):
            p_core_index = i % core_count
            physical_core_map[p_core_index].append(logical_usages[i])

        physical_cores_usage = {
            f"physical_core_{i+1}": round(sum(usages) / len(usages), 2)
            for i, usages in physical_core_map.items()
        }

        uuid = self.get_uuid_by_name("cpu_monitoring", "model", self.hardware_identifiers.get('cpu_model', 'Unknown'))
        return {
            "cpu_uuid": uuid,
            "p_cores_perc": physical_cores_usage,
            "l_cores_perc": {f"logical_core_{i+1}": usage for i, usage in enumerate(logical_usages)},
            "ctx_switches": cpu_stats.ctx_switches,
            "sw_irq": cpu_stats.soft_interrupts,
            "hw_irq": cpu_stats.interrupts,
            "syscalls": cpu_stats.syscalls,
        }



    def get_disk_info(self):
        result = []
        try:
            io_counters = psutil.disk_io_counters(perdisk=True)

            for disk_name, io in io_counters.items():
               

                uuid = self.get_uuid_by_name("disk_monitoring", "serial_number", disk_name.upper())

                result.append({
                    "disk_uuid": uuid,
                    "read_count_io": io.read_count,
                    "write_count_io": io.write_count,
                    "bytes_read_io": io.read_bytes,
                    "bytes_write_io": io.write_bytes,
                    "read_time_io": io.read_time,
                    "write_time_io": io.write_time
                })

        except Exception as e:
            logging.error(f"Error in get_disk_info: {e}")

        return result


    def partition_monitoring(self):
        partitions_info = []
        for partition in psutil.disk_partitions(all=False):
            if "cdrom" in partition.opts or partition.fstype == "":
                continue
            try:
                mount_point = partition.mountpoint.strip()
                mount_letter = mount_point.strip("\\").rstrip(":") + ":"
                usage = psutil.disk_usage(mount_point)
                uuid, storage_uuid = self.get_uuid_by_name("partition_monitoring","serial_number",self.hardware_identifiers['partition_volume_serials'].get(mount_letter,None))
                partitions_info.append({
                    "partition_uuid": uuid,
                    "disk_uuid": storage_uuid,
                    "mount_point": mount_letter,
                    "free_space": usage.free,
                    "used_space": usage.used,
                    "used_space_perc": f"{usage.percent} %"
                })
            except PermissionError:
                continue
            except Exception as e:
                logging.error(f"Error reading partition {partition.device}: {e}")
                continue
        return partitions_info

    def network_monitoring(self):
        net_info = psutil.net_io_counters(pernic=True)
        net_stats = psutil.net_if_stats()
        network_data = []

        for iface, data in net_info.items():
            stats = net_stats.get(iface)
            if stats and stats.isup and (data.bytes_sent > 0 or data.bytes_recv > 0):
                
                uuid ,nic_uuid= self.get_uuid_by_name("network_monitoring", "interface_name", iface)
                network_data.append({
                    "port_uuid": uuid,
                    "nic_uuid": nic_uuid,
                    "interface": iface,
                    "bytes_sent": data.bytes_sent,
                    "bytes_received": data.bytes_recv,
                    "packets_sent": data.packets_sent,
                    "packets_received": data.packets_recv,
                    "error_in": data.errin,
                    "error_out": data.errout,
                    "drop_in": data.dropin,
                    "drop_out": data.dropout,
                })
        return network_data
if __name__ == "__main__":  
    monitoring = Monitoring()
    checkpoint = monitoring.get_monitoring_checkpoint()
    print(json.dumps(checkpoint, indent=4))    