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


TABLE_UUID_MAP = {
    "memory_monitoring": "memory",
    "cpu_monitoring": "cpu",
    "disk_monitoring": "storage",
    "partition_monitoring": "partition",
    "network_monitoring": "port",
}

class Monitoring:
    def __init__(self):
        # Initialize COM for the main thread
        pythoncom.CoInitialize()
        
        db_path = os.path.abspath(r"D:\ModifiedRust\RustFull\models_database\models_database.sqlite")
        self.engine = create_engine(f"sqlite:///{db_path}")
        Session = sessionmaker(bind=self.engine)
        self.session = Session()
        self.metadata = MetaData()
        self.metadata.reflect(bind=self.engine)
        self.db_path = db_path
        self.uuid_cache = {} 
        self.hardware_identifiers = {
            'memory_serial_number': None,
            'cpu_processor_id': None,
            'disk_serial_number': None,
            'partition_volume_uuids': {},
        }
        self._init_wmi()
        self.cache_hardware_identifiers()

    def _init_wmi(self):
        """Initialize WMI with proper COM initialization"""
        try:
            pythoncom.CoInitialize()  # Initialize COM for the current thread
            self.wmi_obj = wmi.WMI()
        except Exception as e:
            logging.error(f"Failed to initialize WMI: {e}")
            self.wmi_obj = None

    def __del__(self):
        """Cleanup COM when object is destroyed"""
        try:
            pythoncom.CoUninitialize()
        except:
            pass

    def cache_hardware_identifiers(self):
        # Ensure WMI is initialized for the current thread
        if not self.wmi_obj:
            self._init_wmi()
            
        try:
            for mem in self.wmi_obj.Win32_PhysicalMemory():
                self.hardware_identifiers['memory_serial_number'] = mem.SerialNumber.strip()
                break
        except Exception as e:
            logging.error("Error retrieving memory serial number: %s", e)

        try:
            self.hardware_identifiers['cpu_processor_id'] = self.wmi_obj.Win32_Processor()[0].ProcessorId.strip()
        except Exception as e:
            logging.error("Error retrieving CPU ProcessorId: %s", e)

        try:
            for disk in self.wmi_obj.Win32_DiskDrive():
                if disk.SerialNumber:
                    self.hardware_identifiers['disk_serial_number'] = disk.SerialNumber.strip()
                    break
        except Exception as e:
            logging.error("Error retrieving disk serial number: %s", e)

        try:
            for vol in self.wmi_obj.Win32_Volume():
                if vol.DriveLetter:
                    match = re.search(r"Volume{(.+?)}", vol.DeviceID)
                    if match:
                        self.hardware_identifiers['partition_volume_uuids'][vol.DriveLetter.upper()] = match.group(1)
        except Exception as e:
            logging.error("Error retrieving partition volume UUIDs: %s", e)

    def get_uuid_by_name(self, logical_table_name, name_field, name_value):
        if not name_value:
            logging.error(f"Missing name_value for table: {logical_table_name}, field: {name_field}")
            return "unknown"

        cache_key = (logical_table_name, name_value)
        if cache_key in self.uuid_cache:
            return self.uuid_cache[cache_key]

        table_name = TABLE_UUID_MAP.get(logical_table_name)
        if not table_name:
            return "unknown"

        try:
            table = Table(table_name, self.metadata, autoload_with=self.engine)
            result = self.session.query(table.c.uuid).filter(getattr(table.c, name_field) == name_value).first()
            uuid = result[0] if result else "unknown"
        except Exception as e:
            logging.error(f"Error fetching UUID for {name_value}: {e}")
            uuid = "unknown"

        self.uuid_cache[cache_key] = uuid
        return uuid

    def get_monitoring_checkpoint(self):
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        return {
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
        uuid = self.get_uuid_by_name("memory_monitoring", "serial_number", self.hardware_identifiers['memory_serial_number'])
        return {
            "memory_uuid": uuid,
            "memory_used": round((memory_info.total - memory_info.available)),
            "memory_available": round(memory_info.available),
            "total_memory": round(memory_info.total)
        }

    def get_cpu_info(self):
       
            cpu_stats = psutil.cpu_stats()
            logical_usages = psutil.cpu_percent(percpu=True)
            physical_core_map = defaultdict(list)
            logical_to_physical = {}
            core_count = psutil.cpu_count(logical=False)
            logical_count = psutil.cpu_count(logical=True)
            
            
            for i in range(logical_count):
                p_core_index = i % core_count
                physical_core_map[p_core_index].append(logical_usages[i])
 

            physical_cores_usage = {
                f"physical_core_{i+1}": round(sum(usages) / len(usages), 2)
                for i, usages in physical_core_map.items()
            }
            uuid = self.get_uuid_by_name("cpu_monitoring", "os_uuid", self.hardware_identifiers['cpu_processor_id'])
            return {
                "cpu_uuid": uuid,
                "p_cores_perc": physical_cores_usage,  # new field
                "l_cores_perc": {f"logical_core_{i+1}": usage
                    for i, usage in enumerate(logical_usages)},
                "ctx_switches": cpu_stats.ctx_switches,
                "sw_irq": cpu_stats.soft_interrupts,
                "hw_irq":  cpu_stats.interrupts,
                "syscalls": cpu_stats.syscalls,
            }

    def get_disk_info(self):
        total_size = 0
        total_used = 0
        for disk in psutil.disk_partitions(all=True):
            try:
                usage = psutil.disk_usage(disk.mountpoint)
                total_size += usage.total
                total_used += usage.used
            except PermissionError:
                continue

        disk_io = psutil.disk_io_counters()
        uuid = self.get_uuid_by_name("disk_monitoring", "os_uuid", self.hardware_identifiers['disk_serial_number'])

        return {
            "disk_uuid": uuid,
            "total_disk_size": total_size,
            "total_disk_usage": total_used,
            "read_count_io": disk_io.read_count,
            "write_count_io": disk_io.write_count,
            "bytes_read_io": disk_io.read_bytes,
            "bytes_written_io": disk_io.write_bytes,
            "read_time_io_ms": disk_io.read_time,
            "write_time_io_ms": disk_io.write_time
        }

    def partition_monitoring(self):
        partitions_info = []
        for partition in psutil.disk_partitions():
            try:
                mount_point = partition.mountpoint.strip()
                mount_letter = mount_point.strip("\\").rstrip(":") + ":"
                usage = psutil.disk_usage(mount_point)
                uuid = self.get_uuid_by_name("partition_monitoring", "os_uuid", self.hardware_identifiers['partition_volume_uuids'].get(mount_letter))
                partitions_info.append({
                    "partition_uuid": uuid,
                    "mount_point": mount_letter,
                    "free_space": usage.free ,
                    "used_space": usage.used ,
                    "used_space_perc": usage.percent
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
                uuid = self.get_uuid_by_name("network_monitoring", "interface_name", iface)
                network_data.append({
                    "port_uuid": uuid,
                    "interface": iface,
                    "bytes_sent": data.bytes_sent,
                    "bytes_received": data.bytes_recv,
                    "packets_sent": data.packets_sent,
                    "packets_received": data.packets_recv,
                    "error_in": data.errin,
                    "error_out": data.errout,
                    "drop_in": data.dropin,
                    "drop_out": data.dropout
                })

        return network_data
