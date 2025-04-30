-- Your SQL goes here


CREATE TABLE agent_credential (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT NOT NULL,
    client_id TEXT NOT NULL,
    client_secret TEXT NOT NULL,
    master_key TEXT NOT NULL
);

CREATE TABLE agent (
    uuid TEXT PRIMARY KEY,
    os TEXT NOT NULL,
    hostname TEXT NOT NULL
);

CREATE TABLE device (
    uuid TEXT PRIMARY KEY NOT NULL,
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    serial_number TEXT NOT NULL,
    dev_phy_vm TEXT NOT NULL
);

CREATE TABLE cpu (
    uuid TEXT PRIMARY KEY  NOT NULL,
    device_uuid TEXT NOT NULL,
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    p_cores INTEGER NOT NULL,
    l_cores INTEGER NOT NULL,
    speed TEXT NOT NULL,
    os_uuid TEXT,
    FOREIGN KEY (device_uuid) REFERENCES device(uuid)
);

CREATE TABLE memory (
    uuid TEXT PRIMARY KEY  NOT NULL,
    device_uuid TEXT NOT NULL,
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    speed TEXT NOT NULL,
    size TEXT NOT NULL,
    serial_number TEXT NOT NULL,
    os_uuid TEXT,
    FOREIGN KEY (device_uuid) REFERENCES device(uuid)
);

CREATE TABLE storage (
    uuid TEXT PRIMARY KEY  NOT NULL,
    device_uuid TEXT NOT NULL,
    hw_disk_type TEXT NOT NULL,
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    serial_number TEXT NOT NULL,
    base_fs_type TEXT NOT NULL,
    free_space TEXT NOT NULL,
    total_disk_usage TEXT NOT NULL,
    total_disk_size TEXT NOT NULL,
    os_uuid TEXT,
    FOREIGN KEY (device_uuid) REFERENCES device(uuid)
);

CREATE TABLE partition (
    uuid TEXT PRIMARY KEY  NOT NULL,
    storage_uuid TEXT NOT NULL,
    name TEXT NOT NULL,
    fs_type TEXT NOT NULL,
    free_space TEXT NOT NULL,
    used_space TEXT NOT NULL,
    total_size TEXT NOT NULL,
    os_uuid TEXT,
    FOREIGN KEY (storage_uuid) REFERENCES storage(uuid)
);

CREATE TABLE nic (
    uuid TEXT PRIMARY KEY  NOT NULL,
    device_uuid TEXT NOT NULL,
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    number_of_ports INTEGER NOT NULL,
    max_speed TEXT NOT NULL,
    supported_speeds TEXT NOT NULL,
    serial_number TEXT NOT NULL,
    os_uuid TEXT,
    FOREIGN KEY (device_uuid) REFERENCES device(uuid)
);

CREATE TABLE port (
    uuid TEXT PRIMARY KEY  NOT NULL,
    nic_uuid TEXT NOT NULL,
    interface_name TEXT NOT NULL,
    operating_speed TEXT NOT NULL,
    is_physical_logical TEXT NOT NULL,
    logical_type TEXT NOT NULL,
    os_uuid TEXT,
    FOREIGN KEY (nic_uuid) REFERENCES nic(uuid)
);

CREATE TABLE ip_address (
    uuid TEXT PRIMARY KEY  NOT NULL,
    port_uuid TEXT NOT NULL,
    address TEXT NOT NULL,
    gateway TEXT,
    subnet_mask TEXT NOT NULL,
    dns TEXT NOT NULL,
    os_uuid TEXT,
    FOREIGN KEY (port_uuid) REFERENCES port(uuid)
);

CREATE TABLE gpu (
    uuid TEXT PRIMARY KEY  NOT NULL,
    device_uuid TEXT NOT NULL,
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    serial_number TEXT NOT NULL,
    size TEXT NOT NULL,
    driver TEXT NOT NULL,
    os_uuid TEXT,
    FOREIGN KEY (device_uuid) REFERENCES device(uuid)
);

CREATE TABLE tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    token TEXT NOT NULL ,
    expiration TEXT NOT NULL,
    token_type TEXT NOT NULL UNIQUE 
);

-- Your SQL goes here
