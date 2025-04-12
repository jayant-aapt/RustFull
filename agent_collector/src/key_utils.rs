use aes_gcm::aead::rand_core::OsRng;
use base64::engine::general_purpose;
use base64::Engine as _;
use rand::RngCore;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use windows::Win32::Foundation::HLOCAL;
use windows::Win32::Security::Credentials::{
    CredReadW, CredWriteW, CREDENTIALW, CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_LOCAL_MACHINE,
};
use windows::Win32::System::Memory::LocalFree;
use windows::core::{PCWSTR, PWSTR};

const CRED_KEY_NAME: &str = "MasterKeyStore";

pub struct KeyManager;

impl KeyManager {
    #[allow(dead_code)] // Suppress warning for unused function
    pub fn mark_as_onboarded() {
        let onboard_file = Self::get_onboarded_file_path();
        if let Err(e) = fs::write(onboard_file, "onboarded") {
            println!("[ERROR] Failed to mark as onboarded: {}", e);
        }
    }

    #[allow(dead_code)] // Suppress warning for unused function
    pub fn get_onboarded_file_path() -> PathBuf {
        let mut path = dirs::data_dir().expect("Failed to get data directory");
        path.push("onboarded_flag.dat");
        path
    }

    pub fn get_master_key_path() -> PathBuf {
        let mut path = dirs::data_dir().expect("Failed to get data directory");
        path.push("master_keyyys.dat");
        path
    }

    pub fn generate_master_key() -> Vec<u8> {
        let mut master_key = vec![0u8; 32];
        OsRng.fill_bytes(&mut master_key);

        let encrypted_master_key = Self::encrypt_with_dpapi(&master_key);
        let path = Self::get_master_key_path();

        fs::create_dir_all(path.parent().unwrap()).expect("Failed to create directories");
        let mut file = File::create(path).expect("Failed to create master key file");
        file.write_all(&encrypted_master_key)
            .expect("Failed to write master key");

        Self::store_in_windows_cred(&master_key);
        println!("[INFO] New master key generated and stored securely.");
        master_key
    }

    pub fn load_master_key() -> Vec<u8> {
        if let Some(key) = Self::read_from_windows_cred() {
            println!("[INFO] Master key loaded from Windows Credential Manager.");
            return key;
        }

        if let Ok(encoded_key) = env::var("ENCRYPT_MASTER_KEY") {
            if let Ok(decoded_key) = general_purpose::STANDARD.decode(encoded_key) {
                println!("[INFO] Master key loaded from OS Environment Variable.");
                return decoded_key;
            }
        }

        let path = Self::get_master_key_path();
        if path.exists() {
            let mut file = File::open(&path).expect("Failed to open master key file");
            let mut encrypted_master_key = Vec::new();
            file.read_to_end(&mut encrypted_master_key)
                .expect("Failed to read master key");
            let decrypted_key = Self::decrypt_with_dpapi(&encrypted_master_key);
            println!("[INFO] Master key loaded from file.");
            return decrypted_key;
        }

        println!("[WARNING] Master key not found, generating a new one.");
        Self::generate_master_key()
    }

    pub fn encrypt_with_dpapi(data: &[u8]) -> Vec<u8> {
        unsafe {
            let data_blob = CRYPT_INTEGER_BLOB {
                cbData: data.len() as u32,
                pbData: data.as_ptr() as *mut u8,
            };

            let mut encrypted_blob = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };

            let success = CryptProtectData(
                &data_blob,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_LOCAL_MACHINE,
                &mut encrypted_blob,
            );

            if success.as_bool() {
                let slice =
                    std::slice::from_raw_parts(encrypted_blob.pbData, encrypted_blob.cbData as usize);
                let result = slice.to_vec();
                let _ = LocalFree(HLOCAL(encrypted_blob.pbData as isize)); // Handle unused Result
                result
            } else {
                panic!("DPAPI encryption failed");
            }
        }
    }

    pub fn decrypt_with_dpapi(encrypted_data: &[u8]) -> Vec<u8> {
        unsafe {
            let encrypted_blob = CRYPT_INTEGER_BLOB {
                cbData: encrypted_data.len() as u32,
                pbData: encrypted_data.as_ptr() as *mut u8,
            };

            let mut decrypted_blob = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };

            let success = CryptUnprotectData(
                &encrypted_blob,
                None,
                None,
                None,
                None,
                0,
                &mut decrypted_blob,
            );

            if success.as_bool() {
                let slice =
                    std::slice::from_raw_parts(decrypted_blob.pbData, decrypted_blob.cbData as usize);
                let result = slice.to_vec();
                let _ = LocalFree(HLOCAL(decrypted_blob.pbData as isize)); // Handle unused Result
                result
            } else {
                panic!("DPAPI decryption failed");
            }
        }
    }

    pub fn store_in_windows_cred(key: &[u8]) {
        let key_utf16: Vec<u16> = CRED_KEY_NAME.encode_utf16().chain([0]).collect();
        let target_name = PWSTR(key_utf16.as_ptr() as *mut _);

        let cred = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: target_name,
            Comment: PWSTR::null(),
            LastWritten: Default::default(),
            CredentialBlobSize: key.len() as u32,
            CredentialBlob: key.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: PWSTR::null(),
            UserName: PWSTR::null(),
        };

        unsafe {
            let result = CredWriteW(&cred, 0);
            if !result.as_bool() {
                println!("[ERROR] Failed to store master key in Windows Credential Manager.");
            }
        }
    }

    pub fn read_from_windows_cred() -> Option<Vec<u8>> {
        let key_utf16: Vec<u16> = CRED_KEY_NAME.encode_utf16().chain([0]).collect();
        let credential_name = PCWSTR(key_utf16.as_ptr());

        let mut pcred: *mut CREDENTIALW = std::ptr::null_mut();

        unsafe {
            let result = CredReadW(credential_name, CRED_TYPE_GENERIC.0, 0, &mut pcred);
            if result.as_bool() {
                let cred = &*pcred;
                let key_slice = std::slice::from_raw_parts(
                    cred.CredentialBlob,
                    cred.CredentialBlobSize as usize,
                );
                Some(key_slice.to_vec())
            } else {
                None
            }
        }
    }
}
