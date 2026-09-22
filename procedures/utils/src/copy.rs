use doze::{Procedure, ProcedureError, ProcedureRegistration, Rule};
use std::fs;

pub struct Copy;

impl Procedure for Copy {
    fn exec(&mut self, rule: &mut Rule) -> Result<(), ProcedureError> {
        if rule.input_tags().len() != rule.output_tags().len() {
            return Err(ProcedureError::ExecFailed(format!(
                "received unequal amount of input and output artifacts: {} and {}",
                rule.input_tags().len(),
                rule.output_tags().len()
            )));
        }
        for (input, output) in rule.input_tags().iter().zip(rule.output_tags().iter()) {
            fs::copy(&input.0, &output.0)
                .map_err(|e| ProcedureError::ExecFailed(format!("copy failed: {e}")))?;
        }
        Ok(())
    }
}

inventory::submit! {
    ProcedureRegistration {
        id: "utils:copy",
        factory: || Box::new(Copy),
    }
}
