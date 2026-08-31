//! Linux procfs process discovery.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Read-only subset of procfs used to determine local Codex activity.
///
/// Implementations deliberately expose no process control operations. A
/// caller that cannot inspect a process must treat it as unknown, never as
/// active.
pub trait ProcReader {
    fn read_cmdline(&self, pid: u32) -> io::Result<Vec<String>>;
    fn read_parent(&self, pid: u32) -> io::Result<u32>;
    fn read_children(&self, pid: u32) -> io::Result<Vec<u32>>;
}

/// A real procfs reader rooted at `/proc`.
#[derive(Debug, Clone)]
pub struct ProcfsReader {
    root: PathBuf,
}

impl ProcfsReader {
    pub fn system() -> Self {
        Self::new("/proc")
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn process_path(&self, pid: u32, suffix: &str) -> PathBuf {
        self.root.join(pid.to_string()).join(suffix)
    }
}

impl Default for ProcfsReader {
    fn default() -> Self {
        Self::system()
    }
}

impl ProcReader for ProcfsReader {
    fn read_cmdline(&self, pid: u32) -> io::Result<Vec<String>> {
        let bytes = fs::read(self.process_path(pid, "cmdline"))?;
        Ok(bytes
            .split(|byte| *byte == 0)
            .filter(|argument| !argument.is_empty())
            .map(|argument| String::from_utf8_lossy(argument).into_owned())
            .collect())
    }

    fn read_parent(&self, pid: u32) -> io::Result<u32> {
        let status = fs::read_to_string(self.process_path(pid, "status"))?;
        status
            .lines()
            .find_map(|line| line.strip_prefix("PPid:\t"))
            .and_then(|parent| parent.trim().parse().ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "procfs PPid is missing"))
    }

    fn read_children(&self, pid: u32) -> io::Result<Vec<u32>> {
        let children = fs::read_to_string(
            self.process_path(pid, "task")
                .join(pid.to_string())
                .join("children"),
        )?;
        Ok(children
            .split_whitespace()
            .filter_map(|child| child.parse().ok())
            .collect())
    }
}

/// A discovered process in a Codex-owned process tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxProcess {
    pub pid: u32,
    pub parent_pid: u32,
    pub command_line: Vec<String>,
}

/// Discover running Codex roots and their descendants using read-only procfs
/// facts. Permission failures and malformed proc files produce an empty or
/// partial result rather than a false busy signal.
pub fn discover_codex_processes(reader: &impl ProcReader) -> Vec<LinuxProcess> {
    let mut visited = HashSet::new();
    let mut queue = reader.read_children(1).unwrap_or_default();
    let mut discovered = Vec::new();
    let mut codex_processes = HashSet::new();

    while let Some(pid) = queue.pop() {
        if !visited.insert(pid) {
            continue;
        }

        let children = reader.read_children(pid).unwrap_or_default();
        queue.extend(children);

        let command_line = match reader.read_cmdline(pid) {
            Ok(command_line) if !command_line.is_empty() => command_line,
            _ => continue,
        };
        let parent_pid = reader.read_parent(pid).unwrap_or_default();
        let is_codex_root = is_codex_command(&command_line);
        let belongs_to_codex = is_codex_root || codex_processes.contains(&parent_pid);
        if !belongs_to_codex {
            continue;
        }
        codex_processes.insert(pid);
        discovered.push(LinuxProcess {
            pid,
            parent_pid,
            command_line,
        });
    }

    discovered.sort_by_key(|process| process.pid);
    discovered
}

fn is_codex_command(command_line: &[String]) -> bool {
    command_line
        .first()
        .and_then(|program| Path::new(program).file_name())
        .is_some_and(|name| name == "codex")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io;

    use super::{ProcReader, discover_codex_processes};

    struct FixtureProcReader {
        processes: BTreeMap<u32, (String, u32)>,
    }

    impl FixtureProcReader {
        fn from(entries: impl IntoIterator<Item = (u32, &'static str, u32)>) -> Self {
            Self {
                processes: entries
                    .into_iter()
                    .map(|(pid, command, parent)| (pid, (command.to_owned(), parent)))
                    .collect(),
            }
        }
    }

    impl ProcReader for FixtureProcReader {
        fn read_cmdline(&self, pid: u32) -> io::Result<Vec<String>> {
            self.processes
                .get(&pid)
                .map(|(command, _)| command.split_whitespace().map(str::to_owned).collect())
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }

        fn read_parent(&self, pid: u32) -> io::Result<u32> {
            self.processes
                .get(&pid)
                .map(|(_, parent)| *parent)
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }

        fn read_children(&self, pid: u32) -> io::Result<Vec<u32>> {
            Ok(self
                .processes
                .iter()
                .filter_map(|(child, (_, parent))| (*parent == pid).then_some(*child))
                .collect())
        }
    }

    #[test]
    fn proc_reader_discovers_codex_and_app_server_children() {
        let reader =
            FixtureProcReader::from([(100, "codex --serve", 1), (101, "codex app-server", 100)]);

        let processes = discover_codex_processes(&reader);

        assert_eq!(
            processes
                .iter()
                .map(|process| process.pid)
                .collect::<Vec<_>>(),
            vec![100, 101]
        );
    }

    #[test]
    fn proc_reader_keeps_nested_children_of_a_codex_process() {
        let reader = FixtureProcReader::from([
            (100, "codex --serve", 1),
            (101, "codex app-server", 100),
            (102, "node worker.js", 101),
        ]);

        let processes = discover_codex_processes(&reader);

        assert_eq!(
            processes
                .iter()
                .map(|process| process.pid)
                .collect::<Vec<_>>(),
            vec![100, 101, 102]
        );
    }

    #[test]
    fn unreadable_proc_entries_are_ignored_instead_of_reported_as_activity() {
        struct DeniedReader;

        impl ProcReader for DeniedReader {
            fn read_cmdline(&self, _pid: u32) -> io::Result<Vec<String>> {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }

            fn read_parent(&self, _pid: u32) -> io::Result<u32> {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }

            fn read_children(&self, _pid: u32) -> io::Result<Vec<u32>> {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
        }

        assert!(discover_codex_processes(&DeniedReader).is_empty());
    }
}
