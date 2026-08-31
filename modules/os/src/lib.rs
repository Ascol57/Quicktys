//! `os` module — certified driver for the operating system: run programs / shell
//! commands and open files or URLs with the default handler (cross-platform,
//! host-side). Holds the `process` capability.

use quicktys_module_sdk::{Capability, Module, ModuleManifest, Schema, ServiceDecl, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",
});

use quicktys::api::process::{open_path, run_shell, spawn};

struct Os;

impl Module for Os {
    fn manifest() -> ModuleManifest {
        ModuleManifest::new("os", "Operating System", "0.1.0")
            .capability(Capability::Process)
            .service(ServiceDecl::new("shell").args(Schema::object([(
                "command".to_string(),
                Schema::string(),
            )])))
            .service(ServiceDecl::new("open").args(Schema::object([(
                "target".to_string(),
                Schema::string(),
            )])))
            .service(ServiceDecl::new("run").args(Schema::object([
                ("program".to_string(), Schema::string()),
                ("args".to_string(), Schema::array(Schema::string())),
            ])))
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        match method {
            "shell" => {
                let cmd = args.get("command").and_then(Value::as_str).unwrap_or_default();
                if cmd.is_empty() {
                    return Err("command is required".to_string());
                }
                run_shell(cmd)?;
                Ok(Value::Null)
            }
            "open" => {
                let target = args.get("target").and_then(Value::as_str).unwrap_or_default();
                if target.is_empty() {
                    return Err("target is required".to_string());
                }
                open_path(target)?;
                Ok(Value::Null)
            }
            "run" => {
                let program = args.get("program").and_then(Value::as_str).unwrap_or_default();
                if program.is_empty() {
                    return Err("program is required".to_string());
                }
                let list: Vec<String> = args
                    .get("args")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                spawn(program, &list)?;
                Ok(Value::Null)
            }
            other => Err(format!("unknown method: {other}")),
        }
    }
}

quicktys_module_sdk::define_module!(Os);
