pub mod calls;

use crate::model::FunctionInfo;
use calls::analyze_calls;

pub fn enrich_with_calls(functions: &mut Vec<FunctionInfo>, source: &str) {
    let call_map = analyze_calls(functions, source);

    // Populate callees and callers
    for (caller_name, callees) in &call_map {
        // Set callees on the caller
        if let Some(caller) = functions.iter_mut().find(|f| &f.name == caller_name) {
            caller.callees = callees.clone();
        }
    }

    // Populate callers (reverse map)
    let fn_names: Vec<String> = functions.iter().map(|f| f.name.clone()).collect();
    for fn_name in &fn_names {
        let callers: Vec<String> = call_map
            .iter()
            .filter(|(_, callees)| callees.contains(fn_name))
            .map(|(caller, _)| caller.clone())
            .collect();

        if let Some(f) = functions.iter_mut().find(|f| &f.name == fn_name) {
            f.callers = callers;
        }
    }
}
