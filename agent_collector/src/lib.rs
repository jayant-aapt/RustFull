use pyo3::prelude::*;
use pyo3::types::PyList;
use shared_config::CONFIG;

/// Collects agent data using a Python module and returns it as a JSON string
pub fn agent_data() -> PyResult<String> {
    pyo3::prepare_freethreaded_python(); 

    Python::with_gil(|py| {
        let sys = py.import("sys")?;
        let path: &PyList = sys.getattr("path")?.downcast()?;
        path.insert(0, format!("{}/agent_collector", CONFIG.app_dir))?;
        let module = py.import("agent_data")?;
        let collector = module.getattr("AgentData")?.call0()?;
        let result = collector.call_method0("collect_data")?;

        let json_module = py.import("json")?;
        let json_str: String = json_module.call_method1("dumps", (result,))?.extract()?;
        
        Ok(json_str)
    })
}