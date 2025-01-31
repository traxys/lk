/// Parses a script file and extracts comments and functions.
use crate::executables::Executable;
use crate::ui::{print_no_functions_in_script_help, print_script_header};
use anyhow::Result;
use pad::{Alignment, PadStr};
use pastel_colours::{GREEN_FG, RESET_FG};
use regex::bytes::Regex;
use std::io::BufRead;
use std::{fs::File, path::Path};

/// Everything we need to know about a function in a script
#[derive(PartialEq, Debug, Clone)]
pub struct Function {
    pub name: String,
    pub comment: Vec<String>,
}

#[derive(PartialEq, Debug, Clone)]
pub struct Script {
    pub path: std::path::PathBuf,
    pub absolute_path: std::path::PathBuf,
    pub comment: Vec<String>,
    pub functions: Vec<Function>,
}

impl Script {
    pub fn new(executable: &Executable) -> Result<Self> {
        let lines = match read_lines(&executable.path) {
            Ok(lines) => lines,
            Err(err) => {
                log::error!(
                    "Unable to read executable: {}. Error was: {err}",
                    &executable.path.to_string_lossy()
                );
                anyhow::bail!("Permissiond denied in opening script");
            }
        };

        // `comments` accumulates comments until we find a function header line, and then they're cleared.
        let mut comments: Vec<String> = Vec::new();
        let mut included_comments: Vec<String> = Vec::new();
        let mut included_functions: Vec<Function> = Vec::new();
        let mut in_header_comments: bool = false;
        for line in lines.flatten() {
            // Find lines that are part of the same comment block
            if line.starts_with('#') {
                // Are we dealing with a hashbang line? If so, then we expect
                // the next line(s) until an empty line to be script comments.
                if line.contains("#!/") {
                    in_header_comments = true;
                } else if in_header_comments {
                    let comment = clean_comment_line(&line);
                    if included_comments.is_empty() && comment.is_empty() {
                        // If we don't yet have any comments, and this comment has 0 length
                        // then we're probably dealing with a spacing line between the hashbang
                        // and the actual file header. So we'll ignore this line.
                    } else {
                        included_comments.push(comment);
                    }
                } else {
                    comments.push(clean_comment_line(&line));
                }
            } else if !line.starts_with('#') {
                // Find lines that start a function
                if is_function_header_line(&line) {
                    let function = get_function(line, &comments);
                    included_functions.push(function);
                }
                comments.clear();
                in_header_comments = false;
            }
        }

        Ok(Self {
            comment: included_comments,
            functions: included_functions,
            path: executable.path.to_owned(),
            absolute_path: executable.absolute_path.to_owned(),
        })
    }

    pub fn get(&self, function_name: &str) -> Option<&Function> {
        self.functions.iter().find(|&n| n.name == function_name)
    }

    pub fn file_name(&self) -> String {
        if self.path.file_name().is_some() {
            self.path.file_name().unwrap().to_string_lossy().to_string()
        } else {
            panic!("File has no name!")
        }
    }

    pub fn path(&self) -> String {
        let mut path = self.path.clone();
        path.pop();
        return path.as_os_str().to_string_lossy().to_string();
    }

    pub fn working_dir_absolute(&self) -> String {
        let mut path = self.absolute_path.clone();
        path.pop();
        return path.as_os_str().to_string_lossy().to_string();
    }

    pub fn pretty_print(&self) {
        print_script_header(self);
        if self.functions.is_empty() {
            print_no_functions_in_script_help();
        } else {
            self.comment.iter().for_each(|comment_line| {
                println!("  {}", comment_line);
            });

            // Get the longest function name
            const INDENT: usize = 2;
            let padding = self
                .functions
                .iter()
                .max_by(|x, y| x.name.len().cmp(&y.name.len()))
                .unwrap() // Will always be Some because the name String must exist.
                .name
                .len()
                + INDENT;
            for function in &self.functions {
                // We'll pad right so everything aligns nicely.
                // First print the function name
                let to_print = function
                    .name
                    .pad_to_width_with_alignment(padding, Alignment::Right);
                let coloured_to_print = format!("{GREEN_FG}{to_print}{RESET_FG}");
                if !function.comment.is_empty() {
                    print!("{coloured_to_print}");
                } else {
                    println!("{coloured_to_print}");
                }

                // Then follow up with the comment lines
                function.comment.iter().enumerate().for_each(|(i, line)| {
                    if i == 0 {
                        println!(" {line}");
                    } else {
                        println!(
                            "{} {line}",
                            "".pad_to_width_with_alignment(padding, Alignment::Right)
                        );
                    }
                });
            }
        }
    }
}

/// Gets a `Function` from a line that contains a function name. Uses accumulated comments.
fn get_function(line: String, comments_found_so_far: &[String]) -> Function {
    let name = line.split("()").next();
    match name {
        Some(actual_name) => Function {
            name: String::from(actual_name.trim()),
            comment: comments_found_so_far
                .iter()
                .map(|comment| comment.to_owned())
                .collect(),
        },
        None => {
            panic!("There is some kind of formatting error with the name of this function:");
        }
    }
}

// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
// https://doc.rust-lang.org/rust-by-example/std_misc/file/read_lines.html
fn read_lines<P>(filename: P) -> std::io::Result<std::io::Lines<std::io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(std::io::BufReader::new(file).lines())
}

fn is_function_header_line(line: &str) -> bool {
    if line.trim().starts_with('_') {
        false
    } else {
        Regex::new(r"^.*\(\).*\{$")
            .unwrap()
            .is_match(line.as_bytes())
    }
}

fn clean_comment_line(line: &str) -> String {
    let mut cleaned = line.trim_start_matches('#');
    cleaned = cleaned.trim_start();
    cleaned.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_comment_line() {
        assert_eq!(clean_comment_line("#First line"), "First line");
        assert_eq!(clean_comment_line("# First line"), "First line");
        assert_eq!(clean_comment_line("# First # line"), "First # line");
        assert_eq!(clean_comment_line("## First # line"), "First # line");
        assert_eq!(clean_comment_line("### First # line"), "First # line");
        assert_eq!(clean_comment_line("### #First # line"), "#First # line");
        assert_eq!(clean_comment_line("#"), "");
        assert_eq!(clean_comment_line("#    "), "");
        assert_eq!(clean_comment_line("#   "), "");
        assert_eq!(clean_comment_line("##   "), "");
    }

    #[test]
    fn test_get_function() {
        // Given
        let line = String::from("some_function(){");
        let comments = vec![String::from("First line"), String::from("Second line")];

        // When
        let function = get_function(line, &comments);

        // Then
        assert_eq!(function.name, "some_function");
        assert_eq!(function.comment, vec!["First line", "Second line"]);
    }

    #[test]
    fn test_get_function_edge() {
        // Given
        let line = String::from("   some_function   ()   {");
        let comments = vec![String::from("First line"), String::from("Second # line")];

        // When
        let function = get_function(line, &comments);

        // Then
        assert_eq!(function.name, "some_function");
        assert_eq!(function.comment, vec!["First line", "Second # line"]);
    }

    #[test]
    fn test_is_function_header_line() {
        assert!(is_function_header_line(&String::from("some_function(){")));
        assert!(is_function_header_line(&String::from(
            "some_function    () {"
        )));
        assert!(is_function_header_line(&String::from(
            "some_function    ()     {"
        )));
        assert!(is_function_header_line(&String::from(
            "    some_function    ()     {"
        )));
    }
}
