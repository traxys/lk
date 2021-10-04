use colored::Colorize;

use crate::script::Function;
use anyhow::Result;

use std::process::Command;
use std::process::Stdio;

use crate::script::Script;
use nanoid::nanoid;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

pub struct BashFile {
    location: String,
    script: Script,
    function: Function,
}

impl BashFile {
    pub fn new(script: Script, function: Function) -> Self {
        Self {
            location: format!("./~lk_{}", nanoid!(10)),
            script,
            function,
        }
    }

    /// lk uses a temporary file in order to execute a function in a script. This temporary file
    /// sources the script we're going to execute and then it can run the function because it'll
    /// have been loaded into the shell. `std::process::Command` has no way to do this. An alternative
    /// would be adding `"$@"` to the end of the scripts but I'd rather avoid this stipulation.
    pub fn write(&self) -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o700)
            .open(&self.location)?;
        let bash_file = r#"#!/usr/bin/env bash
# 
# Temporary lk file used to execute functions in scripts.
# If you see it here you can delete it and/or gitignore it.

"#;
        writeln!(
            file,
            "{} source {} && {}",
            bash_file,
            self.script.path(),
            self.function.name
        )?;
        Ok(())
    }

    /// This executes the lk file, and then removes it.
    pub fn execute(&self) -> Result<()> {
        println!(
            "{}{}{}{}",
            "lk: ".on_blue(),
            self.script.path.as_os_str().to_string_lossy().on_blue(),
            " -> ".on_blue(),
            self.function.name.on_blue()
        );
        let mut cmd = Command::new(&self.location)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let exit_status = cmd.wait()?;
        match exit_status.code() {
            Some(code) => {
                match std::fs::remove_file(&self.location) {
                    Ok(_) => {
                        // Great, we've tidied up.
                    }
                    Err(e) => {
                        if e.to_string().contains("No such file or directory") {
                            // We don't care about this
                        } else {
                            eprintln!(
                            "Yikes! I couldn't remove my temporary file, '{}'! The error was {}",
                            self.location,
                            e.to_string().red()
                        )
                        }
                    }
                }
                std::process::exit(code)
            }
            None => eprintln!("Your function exited without a status code!"),
        }
        Ok(())
    }
}