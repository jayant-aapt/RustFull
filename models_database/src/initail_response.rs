use diesel::prelude::*;
use diesel::result::Error;
use serde_json::Value;
use crate::models::*;
use crate::schema::*;
use anyhow::{Result, Context};

pub fn store_json_data(conn: &mut SqliteConnection, json_data: &Value) -> Result<(), Error> {
    conn.transaction::<_, Error, _>(|conn| {
        println!("Storing JSON data into the database...");
        println!("JSON data: {:?}", json_data.to_string());
        // Insert Agent
        if let Some(agent_value) = json_data.get("agent") {
            let agent: Agent = serde_json::from_value(agent_value.clone())
                .map_err(|e| {
                    println!("Failed to parse agent: {e}");
                    Error::RollbackTransaction
                })?;
            diesel::insert_into(agent::table).values(&agent).execute(conn)
                .map_err(|e| {
                    println!("Failed to insert into agent table: {e}");
                    Error::RollbackTransaction
                })?;
        }

        // Insert Device and Related Tables
        if let Some(device_value) = json_data.get("device") {
            let device: Device = serde_json::from_value(device_value.clone())
                .map_err(|e| {
                    println!("Failed to parse device: {e}");
                    Error::RollbackTransaction
                })?;
            diesel::insert_into(device::table).values(&device).execute(conn)
                .map_err(|e| {
                    println!("Failed to insert into device table: {e}");
                    Error::RollbackTransaction
                })?;

            let device_uuid = device.uuid.clone();

            // CPU
            if let Some(cpu_array) = device_value.get("cpu").and_then(|v| v.as_array()) {
                for c in cpu_array {
                    let mut cpu: Cpu = serde_json::from_value(c.clone()).map_err(|e| {
                        println!("Failed to parse CPU: {e}");
                        Error::RollbackTransaction
                    })?;
                    cpu.device_uuid = device_uuid.clone();
                    diesel::insert_into(cpu::table).values(&cpu).execute(conn).map_err(|e| {
                        println!("Failed to insert into cpu table: {e}");
                        Error::RollbackTransaction
                    })?;
                }
            }

            // Memory
            if let Some(mem_array) = device_value.get("memory").and_then(|v| v.as_array()) {
                for m in mem_array {
                    let mut memory: Memory = serde_json::from_value(m.clone()).map_err(|e| {
                        println!("Failed to parse memory: {e}");
                        Error::RollbackTransaction
                    })?;
                    memory.device_uuid = device_uuid.clone();
                    diesel::insert_into(memory::table).values(&memory).execute(conn).map_err(|e| {
                        println!("Failed to insert into memory table: {e}");
                        Error::RollbackTransaction
                    })?;
                }
            }

            // Storage + Partition
            if let Some(stor_array) = device_value.get("storage").and_then(|v| v.as_array()) {
                for s in stor_array {
                    let mut storage: Storage = serde_json::from_value(s.clone()).map_err(|e| {
                        println!("Failed to parse storage: {e}");
                        Error::RollbackTransaction
                    })?;
                    storage.device_uuid = device_uuid.clone();
                    let storage_uuid = storage.uuid.clone();
                    diesel::insert_into(storage::table).values(&storage).execute(conn).map_err(|e| {
                        println!("Failed to insert into storage table: {e}");
                        Error::RollbackTransaction
                    })?;

                    if let Some(part_array) = s.get("partition").and_then(|v| v.as_array()) {
                        for p in part_array {
                            let mut partition: Partition = serde_json::from_value(p.clone()).map_err(|e| {
                                println!("Failed to parse partition: {e}");
                                Error::RollbackTransaction
                            })?;
                            partition.storage_uuid = storage_uuid.clone();
                            diesel::insert_into(partition::table).values(&partition).execute(conn).map_err(|e| {
                                println!("Failed to insert into partition table: {e}");
                                Error::RollbackTransaction
                            })?;
                        }
                    }
                }
            }

            // NIC + Port + IP
            if let Some(nics) = device_value.get("nic").and_then(|v| v.as_array()) {
                for n in nics {
                    let mut nic: Nic = serde_json::from_value(n.clone()).map_err(|e| {
                        println!("Failed to parse NIC: {e}");
                        Error::RollbackTransaction
                    })?;
                    nic.device_uuid = device_uuid.clone();
                    let nic_uuid = nic.uuid.clone();
                    diesel::insert_into(nic::table).values(&nic).execute(conn).map_err(|e| {
                        println!("Failed to insert into nic table: {e}");
                        Error::RollbackTransaction
                    })?;

                    if let Some(port_array) = n.get("port").and_then(|v| v.as_array()) {
                        for port_v in port_array {
                            let mut port: Port = serde_json::from_value(port_v.clone()).map_err(|e| {
                                println!("Failed to parse port: {e}");
                                Error::RollbackTransaction
                            })?;
                            port.nic_uuid = nic_uuid.clone();
                            let port_uuid = port.uuid.clone();
                            diesel::insert_into(port::table).values(&port).execute(conn).map_err(|e| {
                                println!("Failed to insert into port table: {e}");
                                Error::RollbackTransaction
                            })?;

                            if let Some(ip_array) = port_v.get("ip").and_then(|v| v.as_array()) {
                                for ip_v in ip_array {
                                    let mut ip: Ip = serde_json::from_value(ip_v.clone()).map_err(|e| {
                                        println!("Failed to parse IP: {e}");
                                        Error::RollbackTransaction
                                    })?;
                                    ip.port_uuid = port_uuid.clone();
                                    diesel::insert_into(ip_address::table).values(&ip).execute(conn).map_err(|e| {
                                        println!("Failed to insert into ip_address table: {e}");
                                        Error::RollbackTransaction
                                    })?;
                                }
                            }
                        }
                    }
                }
            }

            // GPU
            if let Some(gpus) = device_value.get("gpu").and_then(|v| v.as_array()) {
                for g in gpus {
                    let mut gpu: Gpu = serde_json::from_value(g.clone()).map_err(|e| {
                        println!("Failed to parse GPU: {e}");
                        Error::RollbackTransaction
                    })?;
                    gpu.device_uuid = device_uuid.clone();
                    diesel::insert_into(gpu::table).values(&gpu).execute(conn).map_err(|e| {
                        println!("Failed to insert into gpu table: {e}");
                        Error::RollbackTransaction
                    })?;
                }
            }
        }

        println!("JSON data stored successfully.");
        Ok(())
    })
}


pub fn insert_partition(conn: &mut SqliteConnection,data: &Value, uuid: &str ) -> Result<()> {

    let mut partition_data: Partition = serde_json::from_value(data.clone())
    .context("Failed to parse partition data from JSON")?;
    let existing_partition = partition::table
        .filter(partition::storage_uuid.eq(uuid)) 
        .first::<Partition>(conn)
        .optional()?;  

    match existing_partition {
        Some(_) => {

            println!("Found matching storage_uuid, proceeding to insert partition.");

            diesel::insert_into(partition::table)
                .values(&partition_data)  
                .execute(conn)
                .map_err(|e| {
                    println!("Failed to insert into partition table: {e}");
                    e
                })?;

            println!("Partition data inserted successfully.");
            Ok(())
        }
        None => {
            println!("No matching storage_uuid found, skipping partition insertion.");
            Ok(())
        }
    }
}