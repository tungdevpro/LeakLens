//! Memory and Object Counts from `dumpsys meminfo <pid>`.

use std::collections::BTreeMap;

/// One `meminfo` reading: the App Summary (PSS and RSS columns) and the
/// Objects section, keyed by the labels `meminfo` prints. The App Summary
/// total is stored as `TOTAL`, whichever label the Android version uses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Memory {
    pub pss_kb: BTreeMap<String, u64>,
    pub rss_kb: BTreeMap<String, u64>,
    pub objects: BTreeMap<String, u64>,
}

impl Memory {
    pub fn total_pss_kb(&self) -> Option<u64> {
        self.pss_kb.get("TOTAL").copied()
    }

    pub fn java_heap_kb(&self) -> Option<u64> {
        self.pss_kb.get("Java Heap").copied()
    }

    pub fn native_heap_kb(&self) -> Option<u64> {
        self.pss_kb.get("Native Heap").copied()
    }

    pub fn graphics_kb(&self) -> Option<u64> {
        self.pss_kb.get("Graphics").copied()
    }

    pub fn activities(&self) -> Option<u64> {
        self.objects.get("Activities").copied()
    }

    pub fn views(&self) -> Option<u64> {
        self.objects.get("Views").copied()
    }

    pub fn app_contexts(&self) -> Option<u64> {
        self.objects.get("AppContexts").copied()
    }

    /// Parses `dumpsys meminfo <pid>` output. Returns `None` when there is no
    /// App Summary, which is what `meminfo` prints for a PID that has gone.
    pub(crate) fn parse(output: &str) -> Option<Self> {
        #[derive(PartialEq)]
        enum Section {
            Other,
            AppSummary,
            Objects,
        }

        let mut memory = Memory::default();
        let mut section = Section::Other;
        let mut found_summary = false;
        // Where the PSS column ends; App Summary values right-aligned past it are RSS.
        let mut pss_column_end = None;

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('-') {
                continue;
            }
            if trimmed == "App Summary" {
                section = Section::AppSummary;
                found_summary = true;
                continue;
            }
            if trimmed == "Objects" {
                section = Section::Objects;
                continue;
            }
            if let Some(start) = line.find("Pss(KB)") {
                pss_column_end = Some(start + "Pss(KB)".len());
                continue;
            }
            if !trimmed.contains(':') {
                // Any other heading starts a section LeakLens doesn't read.
                section = Section::Other;
                continue;
            }
            match section {
                Section::AppSummary if trimmed.starts_with("TOTAL") => {
                    for (label, value) in pairs(line) {
                        match label.as_str() {
                            "TOTAL" | "TOTAL PSS" => memory.pss_kb.insert("TOTAL".into(), value),
                            "TOTAL RSS" => memory.rss_kb.insert("TOTAL".into(), value),
                            _ => memory.pss_kb.insert(label, value),
                        };
                    }
                }
                Section::AppSummary => {
                    let (label, values) = line.split_once(':')?;
                    let label_end = label.len() + 1;
                    for (end, value) in numbers_with_end(values) {
                        let column =
                            if pss_column_end.is_none_or(|pss_end| label_end + end <= pss_end) {
                                &mut memory.pss_kb
                            } else {
                                &mut memory.rss_kb
                            };
                        column.insert(label.trim().to_owned(), value);
                    }
                }
                Section::Objects => memory.objects.extend(pairs(line)),
                Section::Other => {}
            }
        }
        found_summary.then_some(memory)
    }
}

/// `Label: 12   Other label: 34` pairs on one line.
fn pairs(line: &str) -> Vec<(String, u64)> {
    let mut parts = line.split(':');
    let mut label = parts.next().unwrap_or_default().trim().to_owned();
    let mut found = Vec::new();
    for part in parts {
        let part = part.trim_start();
        let (number, rest) = part.split_once(char::is_whitespace).unwrap_or((part, ""));
        if let Ok(value) = number.parse() {
            found.push((label, value));
        }
        label = rest.trim().to_owned();
    }
    found
}

/// The numbers in `text`, each with the offset just past its last digit.
fn numbers_with_end(text: &str) -> Vec<(usize, u64)> {
    let mut found = Vec::new();
    let mut offset = 0;
    for token in text.split(' ') {
        if let Ok(value) = token.parse() {
            found.push((offset + token.len(), value));
        }
        offset += token.len() + 1;
    }
    found
}
