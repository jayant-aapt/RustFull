use pyo3::prelude::*;
use pyo3::types::PyList;
use once_cell::sync::OnceCell;
use shared_config::CONFIG;

// Caches for both AgentData and Monitoring classes
static AGENT_INSTANCE: OnceCell<Py<PyAny>> = OnceCell::new();
static MONITOR_INSTANCE: OnceCell<Py<PyAny>> = OnceCell::new();

fn get_class_instance<'a>(py: Python<'a>,instance_cell: &'a OnceCell<Py<PyAny>>,module_name: &str,class_name: &str,) -> PyResult<&'a Py<PyAny>> {
    instance_cell.get_or_try_init(|| {
        let sys = py.import("sys")?;
        let path: &PyList = sys.getattr("path")?.downcast()?;
        path.insert(0, format!("{}/agent_collector",CONFIG.app_dir))?;

        let module = py.import(module_name)?;
        let class = module.getattr(class_name)?;
        let instance = class.call0()?;
        Ok(instance.into_py(py))
    })
}

fn call_cached_method(instance: &Py<PyAny>, method_name: &str) -> PyResult<String> {
    Python::with_gil(|py| {
        let instance_ref = instance.as_ref(py);  // This should be valid now
        let result = instance_ref.call_method0(method_name)?;

        let json_module = py.import("json")?;
        let json_str: String = json_module.call_method1("dumps", (result,))?.extract()?;
        
        Ok(json_str)
    })
}

fn call_cached_method_with_args(instance: &Py<PyAny>, method_name: &str,arg: &str) -> PyResult<String> {
    Python::with_gil(|py| {
        let instance_ref = instance.as_ref(py);  
        let result = instance_ref.call_method1(method_name,(arg,))?;

        let json_module = py.import("json")?;
        let json_str: String = json_module.call_method1("dumps", (result,))?.extract()?;
        
        Ok(json_str)
    })
}




/// This function scans the partitions using the AgentData class from the agent_data module.
pub fn agent_data() -> PyResult<String> {
    Python::with_gil(|py| {
        let instance = get_class_instance(py, &AGENT_INSTANCE, "agent_data", "AgentData")?;
        let result=call_cached_method(instance, "collect_data");
        result
    })
}
/// This function is used to get the monitoring data from the Monitoring class
pub fn monitor_data() -> PyResult<String> {
    Python::with_gil(|py| {
        let instance = get_class_instance(py, &MONITOR_INSTANCE, "monitoring_data", "Monitoring")?;
        call_cached_method(instance, "get_monitoring_checkpoint")
    })
}



pub fn scan_disk(action: &str) -> PyResult<String> {
    Python::with_gil(|py| {
        let instance = get_class_instance(py, &AGENT_INSTANCE, "agent_data", "AgentData")?;
        let result=call_cached_method_with_args(instance, "scan_particular_action",action);
        result
    })
}

pub fn scan_nic(action: &str) -> PyResult<String> {
    Python::with_gil(|py| {
        let instance = get_class_instance(py, &AGENT_INSTANCE, "agent_data", "AgentData")?;
        let result=call_cached_method_with_args(instance, "scan_particular_action",action);
        result
    })
}