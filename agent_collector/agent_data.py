import psutil # type: ignore
import json
import platform
import logging
import wmi
import uuid
import socket
import subprocess
import requests
import re
import pythoncom 
logging.basicConfig(level=logging.DEBUG, format='%(asctime)s - %(levelname)s - %(message)s')

class AgentData:
  
    def __init__(self):
        pythoncom.CoInitialize()
        self.wmi_obj = wmi.WMI()
        
    def __del__(self):
        """Cleanup COM when object is destroyed"""
        try:
            pythoncom.CoUninitialize()
        except:
            pass
    
    def get_cpu_details(self):
        try:
            cpu_info = self.wmi_obj.Win32_Processor()[0]
            return [{
                "os_uuid":cpu_info.ProcessorId,
                "make": cpu_info.Manufacturer.strip(),
                "model": cpu_info.Name.strip(),
                "p_cores": psutil.cpu_count(logical=False),
                "l_cores": psutil.cpu_count(logical=True),
                "speed": psutil.cpu_freq().max
            }]
        except Exception as e:
            logging.error("Error retrieving CPU details: %s", e)
            return []
    
    def get_memory_details(self):
        try:
            memory_data = []
            for mem in self.wmi_obj.Win32_PhysicalMemory():
                memory_data.append({
                    "make": mem.Manufacturer.strip(),
                    "model": mem.PartNumber.strip(),
                    "speed":mem.Speed,
                    "size": int(mem.Capacity),
                    "serial_number": mem.SerialNumber.strip()
                })
            return memory_data
        except Exception as e:
            logging.error("Error retrieving memory details: %s", e)
            return []
   

    def get_partitions(self):
        try:
            logical_disks = {ld.DeviceID: ld for ld in self.wmi_obj.Win32_LogicalDisk()}
            volumes = {vol.DeviceID: vol for vol in self.wmi_obj.Win32_Volume()}

            partitions = []
            for ld in logical_disks.values():
                volume = next((vol for vol in volumes.values() if vol.DriveLetter == ld.DeviceID), None)
                volume_uuid = None
                if volume:
                    match = re.search(r"Volume{(.+?)}", volume.DeviceID)
                    volume_uuid = match.group(1) if match else "UUID Not Found"

                partitions.append({
                    "os_uuid": volume_uuid,
                    "name": ld.DeviceID,
                    "fs_type": ld.FileSystem or "Unknown",
                    "free_space": round(int(ld.FreeSpace)) if ld.FreeSpace else 0,
                    "used_space": round((int(ld.Size or 0)) - int(ld.FreeSpace or 0)),
                    "total_size": round(int(ld.Size)) if ld.Size else 0
                })

            return partitions

        except Exception as e:
            logging.error(f"Error retrieving partition details: {e}")
            return []

    def get_storage_details(self):
        try:
            command = r'powershell -Command "Get-Disk | Select-Object UniqueId | ConvertTo-Json"'
            storage_uuid = subprocess.check_output(command, shell=True, universal_newlines=True)
            data = json.loads(storage_uuid)
            unique_id = data["UniqueId"].strip().split()[-1]

            storage_data = []
            partitions = self.get_partitions()  # Get all partitions

            total_free_space = sum(p["free_space"] for p in partitions)
            total_used_space = sum(p["used_space"] for p in partitions)
            total_size = sum(p["total_size"] for p in partitions)

            for disk in self.wmi_obj.Win32_DiskDrive():
                storage_data.append({
                    "os_uuid": unique_id,
                    "hw_disk_type": "sata",
                    "make": disk.Manufacturer.strip() if disk.Manufacturer else "Unknown",
                    "model": disk.Model.strip() if disk.Model else "Unknown",
                    "serial_number": disk.SerialNumber.strip() if disk.SerialNumber else "Unknown",
                    "base_fs_type": partitions[0]["fs_type"] if partitions else "Unknown",
                    "free_space": total_free_space,
                    "total_disk_usage": total_used_space,
                    "total_disk_size": total_size,
                    "partition": partitions
                })

            return storage_data

        except Exception as e:
            logging.error(f"Error retrieving storage details: {e}")
            return []


    def get_network_details(self):
        try:
            network_data = []
            for nic in self.wmi_obj.Win32_NetworkAdapter(NetEnabled=True):
                ip_info = []

                for nic_config in self.wmi_obj.Win32_NetworkAdapterConfiguration(IPEnabled=True):
                    if nic_config.Description == nic.Description:  
                        ip_info.append({
                            "address": nic_config.IPAddress[0] if nic_config.IPAddress else "Unknown",
                            "gateway": nic_config.DefaultIPGateway[0] if nic_config.DefaultIPGateway else "Unknown",
                            "subnet_mask": nic_config.IPSubnet[0] if nic_config.IPSubnet else "Unknown",
                            "dns": nic_config.DNSServerSearchOrder[0] if nic_config.DNSServerSearchOrder else "Unknown"
                        })
                try:
                    max_speed_bps = int(getattr(nic, 'Speed', 0))
                except ValueError:
                    max_speed_bps = 0 

                if max_speed_bps > 0:
                    max_speed =max_speed_bps
                else:
                    max_speed = "Unknown"

                serial_number = getattr(nic, 'PNPDeviceID', None)
                logical_ports_count = len([port for port in self.wmi_obj.Win32_NetworkAdapter() if port.NetConnectionID == nic.NetConnectionID])

                network_data.append({
                    "os_uuid":getattr(nic,'GUID','Unknown').strip("{}"),
                    "make": getattr(nic, 'Manufacturer', 'Unknown').strip(),
                    "model": getattr(nic, 'Description', 'Unknown').strip(),
                    "number_of_ports": logical_ports_count,
                    "max_speed": max_speed,
                    "supported_speeds": "1000, 2500",  
                    "serial_number": serial_number.strip(),
                    "port": [{
                        "interface_name": getattr(nic, 'NetConnectionID', 'Unknown').strip(),
                        "operating_speed": int(getattr(nic, 'Speed', 0)),
                        "is_physical_logical": "physical",
                        "logical_type": "bridge",  
                        "ip": ip_info
                    }]
                })
            return network_data
        except Exception as e:
            logging.error("Error retrieving network details: %s", e)
            return []


    
    def get_gpu_details(self):
            try:
                gpu_data = []
                for gpu in self.wmi_obj.Win32_VideoController():
                    serial_number = getattr(gpu, 'PNPDeviceID', None)
                    if not serial_number: 
                        serial_number = getattr(gpu, 'DeviceID', 'Unknown')

                    gpu_data.append({
                        "make": gpu.AdapterCompatibility.strip() if gpu.AdapterCompatibility else 'Unknown',
                        "model": gpu.Name.strip() if gpu.Name else 'Unknown',
                        "serial_number": serial_number.strip() if serial_number else 'Unknown',
                        "size": int(gpu.AdapterRAM) if gpu.AdapterRAM else "Unknown",
                        "driver": gpu.DriverVersion.strip() if gpu.DriverVersion else "Unknown"
                    })
                return gpu_data
            except Exception as e:
                logging.error(f"Error retrieving GPU details: {e}")
                return []

    def device_details(self):
        try:
            system_info = self.wmi_obj.Win32_ComputerSystem()[0]
            bios_info = self.wmi_obj.Win32_BIOS()[0]
            device_details={
                "make": system_info.Manufacturer.strip(),
                "model": system_info.Model.strip(),
                "serial_number": bios_info.SerialNumber.strip(),
                "dev_phy_vm": "physical",
                "cpu": self.get_cpu_details(),
                "memory": self.get_memory_details(),
                "storage": self.get_storage_details(),
                "nic": self.get_network_details(),
                "gpu": self.get_gpu_details()

            }
            return device_details

        except Exception as e:
            logging.error(f"Error retrieving GPU details: {e}")
            return []

            

    def collect_data(self):
        try:
          return   {
                "device":self.device_details()

            }
            
        except Exception as e:
            logging.error("Error collecting system data: %s", e)
            return {} 


    