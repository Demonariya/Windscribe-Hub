use crate::models::ProcessState;
use crate::utils::command::CommandRunner;

pub fn detect_windscribe_process(runner: &dyn CommandRunner) -> ProcessState {
    let Ok(output) = runner.run(
        "tasklist",
        &["/FI", "IMAGENAME eq Windscribe.exe", "/FO", "CSV", "/NH"],
    ) else {
        return ProcessState::Unknown;
    };

    if !output.success {
        return ProcessState::Unknown;
    }

    parse_tasklist_csv(&output.stdout)
}

pub fn parse_tasklist_csv(stdout: &str) -> ProcessState {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("windscribe.exe") {
            let cols: Vec<&str> = trimmed
                .split(',')
                .map(|part| part.trim().trim_matches('"'))
                .collect();
            let pid = cols.get(1).and_then(|s| s.parse::<u32>().ok());
            return ProcessState::Running { pid };
        }
    }
    ProcessState::Stopped
}
