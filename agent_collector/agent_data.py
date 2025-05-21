# import psutil # type: ignore
# import json
# import platform
# import logging
# import wmi
# import uuid
# import socket
# import subprocess
# import requests
# import re
# import pythoncom 
# logging.basicConfig(level=logging.DEBUG, format='%(asctime)s - %(levelname)s - %(message)s')

# class AgentData:
  
#     def __init__(self):
#         pythoncom.CoInitialize()
#         self.wmi_obj = wmi.WMI()
        
#     def __del__(self):
#         """Cleanup COM when object is destroyed"""
#         try:
#             pythoncom.CoUninitialize()
#         except:
#             pass
    
#     def get_cpu_details(self):
#         try:
#             cpu_info = self.wmi_obj.Win32_Processor()[0]
#             return [{
#                 # "os_uuid":cpu_info.ProcessorId,
#                 "make": cpu_info.Manufacturer.strip(),
#                 "model": cpu_info.Name.strip(),
#                 "p_cores": psutil.cpu_count(logical=False),
#                 "l_cores": psutil.cpu_count(logical=True),
#                 "speed": psutil.cpu_freq().max
#             }]
#         except Exception as e:
#             logging.error("Error retrieving CPU details: %s", e)
#             return []
    
#     def get_memory_details(self):
#         try:
#             memory_data = []
#             # for mem in self.wmi_obj.Win32_PhysicalMemory():
#             #     memory_data.append({
#             #         "make": mem.Manufacturer.strip(),
#             #         "model": mem.PartNumber.strip(),
#             #         "speed":mem.Speed,
#             #         "size": int(mem.Capacity),
#             #         "serial_number": mem.SerialNumber.strip()
#             #     })
#             # return memory_data
#             for os in self.wmi_obj.Win32_OperatingSystem():
#                 total_virtual_bytes = int(os.TotalVirtualMemorySize)
#                 memory_data.append({
#                     "make": "Microsoft",
#                     "model": "Virtual Memory",
#                     "speed": None,
#                     "size": total_virtual_bytes,
#                     "serial_number": "N/A"
#                 })

#             return memory_data
#         except Exception as e:
#             logging.error("Error retrieving memory details: %s", e)
#             return []
   

#     def get_partitions(self):
#         try:
#             logical_disks = {ld.DeviceID: ld for ld in self.wmi_obj.Win32_LogicalDisk()}
#             volumes = {vol.DeviceID: vol for vol in self.wmi_obj.Win32_Volume()}

#             partitions = []
#             for ld in logical_disks.values():
#                 volume = next((vol for vol in volumes.values() if vol.DriveLetter == ld.DeviceID), None)
#                 volume_uuid = None
#                 if volume:
#                     match = re.search(r"Volume{(.+?)}", volume.DeviceID)
#                     volume_uuid = match.group(1) if match else "UUID Not Found"

#                 partitions.append({
#                     "os_uuid": volume_uuid,
#                     "name": ld.DeviceID,
#                     "fs_type": ld.FileSystem or "Unknown",
#                     "free_space": round(int(ld.FreeSpace)) if ld.FreeSpace else 0,
#                     "used_space": round((int(ld.Size or 0)) - int(ld.FreeSpace or 0)),
#                     "total_size": round(int(ld.Size)) if ld.Size else 0
#                 })

#             return partitions

#         except Exception as e:
#             logging.error(f"Error retrieving partition details: {e}")
#             return []

#     def get_storage_details(self):
#         try:
#             command = r'powershell -Command "Get-Disk | Select-Object UniqueId | ConvertTo-Json"'
#             storage_uuid = subprocess.check_output(command, shell=True, universal_newlines=True)
#             data = json.loads(storage_uuid)
#             unique_id = data["UniqueId"].strip().split()[-1]

#             storage_data = []
#             partitions = self.get_partitions()  # Get all partitions

#             total_free_space = sum(p["free_space"] for p in partitions)
#             total_used_space = sum(p["used_space"] for p in partitions)
#             total_size = sum(p["total_size"] for p in partitions)

#             for disk in self.wmi_obj.Win32_DiskDrive():
#                 storage_data.append({
#                     "os_uuid": unique_id,
#                     "hw_disk_type": "sata",
#                     "make": disk.Manufacturer.strip() if disk.Manufacturer else "Unknown",
#                     "model": disk.Model.strip() if disk.Model else "Unknown",
#                     "serial_number": disk.SerialNumber.strip() if disk.SerialNumber else "Unknown",
#                     "base_fs_type": partitions[0]["fs_type"] if partitions else "Unknown",
#                     "free_space": total_free_space,
#                     "total_disk_usage": total_used_space,
#                     "total_disk_size": total_size,
#                     "partition": partitions
#                 })

#             return storage_data

#         except Exception as e:
#             logging.error(f"Error retrieving storage details: {e}")
#             return []


#     def get_network_details(self):
#         try:
#             network_data = []
#             for nic in self.wmi_obj.Win32_NetworkAdapter(NetEnabled=True):
#                 ip_info = []

#                 for nic_config in self.wmi_obj.Win32_NetworkAdapterConfiguration(IPEnabled=True):
#                     if nic_config.Description == nic.Description:  
#                         ip_info.append({
#                             "address": nic_config.IPAddress[0] if nic_config.IPAddress else "Unknown",
#                             "gateway": nic_config.DefaultIPGateway[0] if nic_config.DefaultIPGateway else "Unknown",
#                             "subnet_mask": nic_config.IPSubnet[0] if nic_config.IPSubnet else "Unknown",
#                             "dns": nic_config.DNSServerSearchOrder[0] if nic_config.DNSServerSearchOrder else "Unknown"
#                         })
#                 try:
#                     max_speed_bps = int(getattr(nic, 'Speed', 0))
#                 except ValueError:
#                     max_speed_bps = 0 

#                 if max_speed_bps > 0:
#                     max_speed =max_speed_bps
#                 else:
#                     max_speed = "Unknown"

#                 serial_number = getattr(nic, 'PNPDeviceID', None)
#                 logical_ports_count = len([port for port in self.wmi_obj.Win32_NetworkAdapter() if port.NetConnectionID == nic.NetConnectionID])

#                 network_data.append({
#                     "os_uuid":getattr(nic,'GUID','Unknown').strip("{}"),
#                     "make": getattr(nic, 'Manufacturer', 'Unknown').strip(),
#                     "model": getattr(nic, 'Description', 'Unknown').strip(),
#                     "number_of_ports": logical_ports_count,
#                     "max_speed": max_speed,
#                     "supported_speeds": "1000, 2500",  
#                     "serial_number": serial_number.strip(),
#                     "port": [{
#                         "interface_name": getattr(nic, 'NetConnectionID', 'Unknown').strip(),
#                         "operating_speed": int(getattr(nic, 'Speed', 0)),
#                         "is_physical_logical": "physical",
#                         "logical_type": "bridge",  
#                         "ip": ip_info
#                     }]
#                 })
#             return network_data
#         except Exception as e:
#             logging.error("Error retrieving network details: %s", e)
#             return []


    
#     def get_gpu_details(self):
#             try:
#                 gpu_data = []
#                 for gpu in self.wmi_obj.Win32_VideoController():
#                     serial_number = getattr(gpu, 'PNPDeviceID', None)
#                     if not serial_number: 
#                         serial_number = getattr(gpu, 'DeviceID', 'Unknown')

#                     gpu_data.append({
#                         "make": gpu.AdapterCompatibility.strip() if gpu.AdapterCompatibility else 'Unknown',
#                         "model": gpu.Name.strip() if gpu.Name else 'Unknown',
#                         "serial_number": serial_number.strip() if serial_number else 'Unknown',
#                         "size": int(gpu.AdapterRAM) if gpu.AdapterRAM else "Unknown",
#                         "driver": gpu.DriverVersion.strip() if gpu.DriverVersion else "Unknown"
#                     })
#                 return gpu_data
#             except Exception as e:
#                 logging.error(f"Error retrieving GPU details: {e}")
#                 return []

#     def device_details(self):
#         try:
#             system_info = self.wmi_obj.Win32_ComputerSystem()[0]
#             bios_info = self.wmi_obj.Win32_BIOS()[0]
#             device_details={
#                 "make": system_info.Manufacturer,
#                 "model": system_info.Model,
#                 "serial_number": bios_info.SerialNumber,
#                 "dev_phy_vm": "physical",
#                 "cpu": self.get_cpu_details(),
#                 "memory": self.get_memory_details(),
#                 "storage": self.get_storage_details(),
#                 "nic": self.get_network_details(),
#                 "gpu": self.get_gpu_details()

#             }
#             return device_details

#         except Exception as e:
#             logging.error(f"Error for collecting agent data : {e}")
#             return []

            

#     def collect_data(self):
#         try:
#           return   {
#                 "device":self.device_details()

#             }
            
#         except Exception as e:
#             logging.error("Error collecting system data: %s", e)
#             return {} 
        
#     def scan_particular_action(self, action):
#         try:
#            if action== "partition":
#                print(action)
#                return{
#                      action: self.get_partitions()
#                }
#         except Exception as e:
#             logging.error(f"Error in scan_particular_action: {e}")
#             return {}


    
import psutil
import json
import logging
import wmi
import subprocess
import re
import os
from sqlalchemy import create_engine, MetaData, Table
from sqlalchemy.orm import sessionmaker
import pythoncom 

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')


class AgentData:
    def __init__(self):
        pythoncom.CoInitialize()
        self.wmi_obj = wmi.WMI()
        db_path = os.path.abspath(r"C:\Users\Administrator\Desktop\Rust_project\RustFull\models_database\models_database.sqlite")
        self.engine = create_engine(f"sqlite:///{db_path}")
        Session = sessionmaker(bind=self.engine)
        self.session = Session()
        self.metadata = MetaData()
        self.metadata.reflect(bind=self.engine)
        self.db_path = db_path
        
    def __del__(self):
        """Cleanup COM when object is destroyed"""
        try:
            pythoncom.CoUninitialize()
        except:
            pass
        
           
    def get_uuid_by_name(self, table_name, name_field, name_value):
        if not name_value:
            logging.error(f"Missing name_value for field: {name_field}")
            return "unknown"

        try:
            table = Table(table_name, self.metadata, autoload_with=self.engine)

            result = self.session.query(table.c.uuid)\
                                .filter(getattr(table.c, name_field) == name_value).first()
            uuid = result[0] if result else "unknown"
            return uuid
        except Exception as e:
            logging.error(f"Error fetching UUID for {name_value}: {e}")
            return "unknown"
   

    def get_cpu_details(self):
        cpu_info = []
        
        try:
            processor = self.wmi_obj.Win32_Processor()
            if processor:
                processor = processor[0]  
                cpu_info.append({
                    "os_uuid": processor.ProcessorId,
                    "make": processor.Manufacturer.strip(),
                    "model": processor.Name.strip(),
                    "p_cores": processor.NumberOfCores,
                    "l_cores": processor.NumberOfLogicalProcessors,
                    "speed": processor.MaxClockSpeed
                
                })
                
            return cpu_info
            
        except Exception as e:
            return [{
                "error": str(e),
                "details": "Failed to get CPU information"
            }]
            

    def get_memory_details(self):
        try:
            memory_info = []
            
            # First try standard physical memory detection
            for mem in self.wmi_obj.Win32_PhysicalMemory():
                if mem.Capacity:  # Only use if we got actual capacity
                    memory_info.append({
                        "make": mem.Manufacturer.strip() if mem.Manufacturer else "Unknown",
                        "model": mem.PartNumber.strip() if mem.PartNumber else "Unknown",
                        "speed": mem.Speed if mem.Speed else 0,
                        "size": int(mem.Capacity),
                        "serial_number": mem.SerialNumber.strip() if mem.SerialNumber else "Unknown"
                    })
            
            # Simple fallback if no memory detected
            if not memory_info:
                for cs in self.wmi_obj.Win32_ComputerSystem():
                    memory_info.append({
                        "make": "Virtual",
                        "model": "System RAM",
                        "speed": 0,
                        "size": int(cs.TotalPhysicalMemory) if cs.TotalPhysicalMemory else 0,
                        "serial_number": "Unknown"
                    })
                    break  # Just need first record
            
            return memory_info if memory_info else [{
                "make": "Unknown",
                "model": "Unknown",
                "speed": 0,
                "size": 0,
                "serial_number": "Unknown"
            }]
            
        except Exception as e:
            logging.error("Error getting memory details: %s", e)
            return [{
                "make": "Error",
                "model": str(e),
                "speed": "Unknown",
                "size": 0,
                "serial_number": "Unknown"
            }]

    # def get_disk_partitions(self, disk, action=None):
    #     partitions = []
    #     try:
    #         volumes = {vol.DriveLetter: vol for vol in self.wmi_obj.Win32_Volume() if vol.DriveLetter}
    #         for partition in disk.associators("Win32_DiskDriveToDiskPartition"):
    #             for logical_disk in partition.associators("Win32_LogicalDiskToPartition"):
    #                 volume = volumes.get(logical_disk.DeviceID)
    #                 volume_uuid = "UUID Not Found"
    #                 if volume and volume.DeviceID:
    #                     match = re.search(r"Volume{(.+?)}", volume.DeviceID)
    #                     if match:
    #                         volume_uuid = match.group(1)

    #                 free = int(logical_disk.FreeSpace or 0)
    #                 size = int(logical_disk.Size or 0)
    #                 used = size - free

    #                 partitions_data={
    #                     "os_uuid": volume_uuid,
    #                     "name": logical_disk.DeviceID,
    #                     "fs_type": logical_disk.FileSystem or "Unknown",
    #                     "free_space": free,
    #                     "used_space": used,
    #                     "total_size": size,
    #                 }
                    
                    
    #                 if action == "disk" or action == "partition":
    #                     uuid=self.get_uuid_by_name("partition", "os_uuid", volume_uuid)
    #                     partitions_data["uuid"]=uuid
                        
    #                 partitions.append(partitions_data)
    #     except Exception as e:
    #         logging.error("Error getting disk partitions: %s", e)
    #     return partitions
 

    def get_disk_partitions(self, disk, action=None):
        partitions = []
        try:
            volumes = {vol.DriveLetter: vol for vol in self.wmi_obj.Win32_Volume() if vol.DriveLetter}
            logical_disks = {ld.DeviceID: ld for ld in self.wmi_obj.Win32_LogicalDisk() if ld.DriveType == 3}

            for partition in disk.associators("Win32_DiskDriveToDiskPartition"):
                for logical_disk in partition.associators("Win32_LogicalDiskToPartition"):
                    volume = volumes.get(logical_disk.DeviceID)
                    logical_disk_obj = logical_disks.get(logical_disk.DeviceID)
                    
                    volume_uuid = "UUID Not Found"
                    volume_serial = "Serial Not Found"

                    # Get UUID from DeviceID
                    if volume and volume.DeviceID:
                        match = re.search(r"Volume{(.+?)}", volume.DeviceID)
                        if match:
                            volume_uuid = match.group(1)

                    # Get Serial Number from LogicalDisk
                    if logical_disk_obj and logical_disk_obj.VolumeSerialNumber:
                        volume_serial = logical_disk_obj.VolumeSerialNumber

                    free = int(logical_disk.FreeSpace or 0)
                    size = int(logical_disk.Size or 0)
                    used = size - free

                    partitions_data = {
                        "os_uuid": volume_uuid,
                        "serial_number": volume_serial,
                        "name": logical_disk.DeviceID,
                        "fs_type": logical_disk.FileSystem or "Unknown",
                        "free_space": free,
                        "used_space": used,
                        "total_size": size,
                    }

                    if action == "disk" or action == "partition":
                        uuid = self.get_uuid_by_name("partition", "os_uuid", volume_uuid)
                        partitions_data["uuid"] = uuid

                    partitions.append(partitions_data)
        except Exception as e:
            logging.error("Error getting disk partitions: %s", e)
        return partitions
    
    def get_storage_details(self , action=None):
        try:
            # Get PowerShell disk UUIDs
            command = r'powershell -Command "Get-Disk | Select-Object Number, FriendlyName, UniqueId | ConvertTo-Json"'
            result = subprocess.check_output(command, shell=True, universal_newlines=True)
            ps_disks = json.loads(result)

            if isinstance(ps_disks, dict):  # Convert single object to list
                ps_disks = [ps_disks]

            # Map disk number to UUID
            disk_uuid_map = {
                d["Number"]: d["UniqueId"].strip().replace(" ", "") if d["UniqueId"] else "Unknown"
                for d in ps_disks
            }

            storage = []

            for disk in self.wmi_obj.Win32_DiskDrive():
                parts = self.get_disk_partitions(disk,action)
                total_free = sum(p["free_space"] for p in parts)
                total_used = sum(p["used_space"] for p in parts)
                total_size = sum(p["total_size"] for p in parts)

                disk_number = disk.Index 

                storage_info={
                    "os_uuid": disk_uuid_map.get(disk_number, "Unknown"),
                    "hw_disk_type": "sata",
                    "make": disk.Manufacturer.strip() if disk.Manufacturer else "Unknown",
                    "model": disk.Model.strip() if disk.Model else "Unknown",
                    "serial_number": disk.DeviceID.split("\\")[-1] if disk.DeviceID else "Unknown",
                    "base_fs_type": parts[0]["fs_type"] if parts else "Unknown",
                    "free_space": total_free,
                    "total_disk_usage": total_used,
                    "total_disk_size": total_size,
                    "partition": parts
                }
                
                if action == "disk" or action=="partition":
                    uuid = self.get_uuid_by_name("storage", "serial_number", disk.DeviceID.split("\\")[-1])
                    storage_info["uuid"] = uuid
                    
                storage.append(storage_info)

            return storage

        except Exception as e:
            logging.error("Error getting storage details: %s", e)
            return []

    def get_network_details(self ,action=None):
        try:
            seen_adapters = {}
    
            # Create a map of NICs with their configurations
            config_map = {cfg.Index: cfg for cfg in self.wmi_obj.Win32_NetworkAdapterConfiguration(IPEnabled=True)}
    
            for nic in self.wmi_obj.Win32_NetworkAdapter(NetEnabled=True):
                if "WAN Miniport" in nic.Description and "Microsoft" in nic.Description:
                    continue
    
                os_uuid = getattr(nic, 'GUID', 'Unknown').strip("{}")
                serial_number = getattr(nic, 'PNPDeviceID', "Unknown")
                key = (os_uuid, serial_number)
    
                # If we have already processed this NIC, just update its port details
                if key in seen_adapters:
                    # Add the current port to the existing NIC entry
                    port_data={
                        "interface_name": getattr(nic, 'NetConnectionID', 'Unknown').strip(),
                        "mac_address": getattr(nic, 'MACAddress', 'Unknown'),
                        "operating_speed": nic.Speed or "Unknown",
                        "is_physical_logical": "physical" if nic.PNPDeviceID else "logical",
                        "logical_type": "bridge" if nic.PNPDeviceID else "virtual",
                        "ip":self.get_ip_data(nic, config_map)
                    }
                    if action== "nic":
                        uuid = self.get_uuid_by_name("port", "interface_name", getattr(nic, 'NetConnectionID', 'Unknown').strip())
                        port_data["uuid"] = uuid
                        
                    seen_adapters[key]["port"].append(port_data)
                    continue  
    
                
                ip_data = self.get_ip_data(nic, config_map , action)
    
                try:
                    speed_bps = int(nic.Speed or 0)
                    max_speed = speed_bps if speed_bps else 0
                except Exception:
                    max_speed = 0
    
                logical_ports_count = len([port for port in self.wmi_obj.Win32_NetworkAdapter() if port.NetConnectionID == nic.NetConnectionID])
    
                seen_adapters[key] = {
                    "os_uuid": os_uuid,
                    "make": nic.Manufacturer.strip() if nic.Manufacturer else "Unknown",
                    "model": nic.Description.strip() if nic.Description else "Unknown",
                    "number_of_ports": logical_ports_count,
                    "max_speed": max_speed,
                    "supported_speeds": "1000, 2500",
                    "serial_number": serial_number,
                    "mac_address": getattr(nic, 'MACAddress', 'Unknown').strip(),
                    "port": [{
                        "interface_name": getattr(nic, 'NetConnectionID', 'Unknown').strip(),
                        "operating_speed": max_speed,
                        "is_physical_logical": "physical" if nic.PNPDeviceID else "logical",
                        "logical_type": "bridge" if nic.PNPDeviceID else "virtual",
                        "ip": ip_data
                    }]
                }
                if action== "nic":
                    seen_adapters[key]["uuid"] = self.get_uuid_by_name("nic", "os_uuid", os_uuid)
                    seen_adapters[key]["port"][0]["uuid"] = self.get_uuid_by_name("port", "interface_name", getattr(nic, 'NetConnectionID', 'Unknown').strip())
    
            return list(seen_adapters.values())
    
        except Exception as e:
            return []
 
    def get_ip_data(self,nic, config_map, action=None):
        """ Helper function to get IP data for a NIC """
        ip_data = []
        cfg = config_map.get(nic.Index)
        if cfg:
            ip={
                "address": cfg.IPAddress[0] if cfg.IPAddress else "Unknown",
                "gateway": cfg.DefaultIPGateway[0] if cfg.DefaultIPGateway else "Unknown",
                "subnet_mask": cfg.IPSubnet[0] if cfg.IPSubnet else "Unknown",
                "dns": cfg.DNSServerSearchOrder[0] if cfg.DNSServerSearchOrder else "Unknown"
            }
                
            ip_data.append(ip)
        return ip_data

    def get_gpu_details(self):
        try:
            gpus = []
            for gpu in self.wmi_obj.Win32_VideoController():
                serial_number = getattr(gpu, 'PNPDeviceID', "Unknown") or getattr(gpu, 'DeviceID', 'Unknown')
                gpus.append({
                    "make": gpu.AdapterCompatibility.strip() if gpu.AdapterCompatibility else 'Unknown',
                    "model": gpu.Name.strip() if gpu.Name else 'Unknown',
                    "serial_number": serial_number.strip() if serial_number else "Unknown",
                    "size": int(gpu.AdapterRAM) if gpu.AdapterRAM else 0,
                    "driver": gpu.DriverVersion.strip() if gpu.DriverVersion else "Unknown"
                })
            return gpus
        except Exception as e:
            logging.error("Error getting GPU details: %s", e)
            return []

    def device_details(self):
        try:
            system = self.wmi_obj.Win32_ComputerSystem()[0]
            bios = self.wmi_obj.Win32_BIOS()[0]
            make=system.Manufacturer.strip() if system.Manufacturer else "Unknown"
            model=system.Model.strip() if system.Model else "Unknown"
            return {
                "make": make,
                "model": model,
                "serial_number": bios.SerialNumber if bios.SerialNumber else "Unknown",
                "dev_phy_vm": "physical" if "vmware" not in system.Model.lower() else "vm",
                "cpu": self.get_cpu_details(),
                "memory": self.get_memory_details(),
                "storage": self.get_storage_details(),
                "nic": self.get_network_details(),
                "gpu": self.get_gpu_details()
            }
        except Exception as e:
            logging.error("Error getting device details: %s", e)
            return {}

    def collect_data(self):
        return {"device": self.device_details()}
    

 
    def scan_particular_action(self, action):
        try:
            if action == "disk" or action == "partition":
                return {
                    "disk": self.get_storage_details(action)
                }
            elif action == "nic":
                return {
                    action: self.get_network_details(action)
                }
           
                
        except Exception as e:
            logging.error(f"Error in scan_particular_action: {e}")
            return {}

if __name__ == "__main__":
    agent_data = AgentData()
    data = agent_data.collect_data()
    print(json.dumps(data, indent=4))
   