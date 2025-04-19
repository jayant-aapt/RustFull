use pyo3::prelude::*;
use pyo3::types::PyList;
<<<<<<< HEAD
use once_cell::sync::OnceCell;
use shared_config::CONFIG;

// Caches for both AgentData and Monitoring classes
static AGENT_INSTANCE: OnceCell<Py<PyAny>> = OnceCell::new();
static MONITOR_INSTANCE: OnceCell<Py<PyAny>> = OnceCell::new();
=======
use shared_config::CONFIG;

/// Collects agent data using a Python module and returns it as a JSON string
pub fn agent_data() -> PyResult<String> {
    pyo3::prepare_freethreaded_python(); 
>>>>>>> 988e83801efc0fc0d06d0d1387e6971d75698051

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


/// This function scans the partitions using the AgentData class from the agent_data module.
pub fn agent_data() -> PyResult<String> {
    Python::with_gil(|py| {
        let instance = get_class_instance(py, &AGENT_INSTANCE, "agent_data", "AgentData")?;
        call_cached_method(instance, "collect_data")
    })
}
/// This function is used to get the monitoring data from the Monitoring class
pub fn monitor_data() -> PyResult<String> {
    Python::with_gil(|py| {
        let instance = get_class_instance(py, &MONITOR_INSTANCE, "monitoring_data", "Monitoring")?;
        call_cached_method(instance, "get_monitoring_checkpoint")
    })
}


// Extra scanning function
pub fn scan_partition() -> PyResult<String> {
    Python::with_gil(|py| {
        let instance = get_class_instance(py, &AGENT_INSTANCE, "agent_data", "AgentData")?;
        call_cached_method(instance, "get_partitions")
    })
}

pub fn scan_disk() -> PyResult<String> {
    Python::with_gil(|py| {
        let instance = get_class_instance(py, &AGENT_INSTANCE, "agent_data", "AgentData")?;
        call_cached_method(instance, "get_disk")
    })
}
