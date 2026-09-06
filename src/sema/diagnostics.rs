// SPDX-License-Identifier: Apache-2.0

use super::ast::{Diagnostic, Level, Namespace};
use crate::file_resolver::FileResolver;
use crate::standard_json::{LocJson, OutputJson};
use codespan_reporting::{diagnostic, files, term};
use itertools::Itertools;
use solang_parser::pt::Loc;
use std::{
    collections::HashMap,
    slice::{Iter, IterMut},
    {io, sync::Arc},
};

#[derive(Default, Debug)]
pub struct Diagnostics {
    contents: Vec<Diagnostic>,
    has_error: bool,
}

impl Diagnostics {
    pub fn any_errors(&self) -> bool {
        self.has_error
    }

    pub fn len(&self) -> usize {
        self.contents.len()
    }

    pub fn iter(&self) -> Iter<'_, Diagnostic> {
        self.contents.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, Diagnostic> {
        self.contents.iter_mut()
    }

    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        if matches!(diagnostic.level, Level::Error) {
            self.has_error = true;
        }
        self.contents.push(diagnostic);
    }

    pub fn extend(&mut self, diagnostics: Diagnostics) {
        self.has_error |= diagnostics.has_error;
        self.contents.extend(diagnostics.contents);
    }

    pub fn append(&mut self, diagnostics: &mut Vec<Diagnostic>) {
        if !self.has_error {
            self.has_error = diagnostics.iter().any(|m| m.level == Level::Error);
        }
        self.contents.append(diagnostics);
    }

    pub fn first_error(&self) -> String {
        match self.contents.iter().find(|m| m.level == Level::Error) {
            Some(m) => m.message.to_owned(),
            None => panic!("no errors found"),
        }
    }

    pub fn count_warnings(&self) -> usize {
        self.contents
            .iter()
            .filter(|&x| x.level == Level::Warning)
            .count()
    }

    pub fn count_errors(&self) -> usize {
        self.contents
            .iter()
            .filter(|&x| x.level == Level::Error)
            .count()
    }

    pub fn first_warning(&self) -> &Diagnostic {
        self.contents
            .iter()
            .find_or_first(|&x| x.level == Level::Warning)
            .unwrap()
    }

    pub fn warnings(&self) -> Vec<&Diagnostic> {
        let mut res = Vec::new();
        for elem in &self.contents {
            if elem.level == Level::Warning {
                res.push(elem);
            }
        }

        res
    }

    pub fn errors(&self) -> Vec<&Diagnostic> {
        let mut vec = Vec::new();
        for diag in &self.contents {
            if matches!(diag.level, Level::Error) {
                vec.push(diag);
            }
        }
        vec
    }

    pub fn warning_contains(&self, message: &str) -> bool {
        let warnings = self.warnings();
        for warning in warnings {
            if warning.message == message {
                return true;
            }
        }

        false
    }

    pub fn contains_message(&self, message: &str) -> bool {
        for item in &self.contents {
            if item.message == message {
                return true;
            }
        }

        false
    }

    // We may have duplicate entries. Also ensure diagnostics are give in order
    // of location
    pub fn sort_and_dedup(&mut self) {
        self.contents.sort();
        self.contents.dedup();
    }
}

fn convert_diagnostic(
    msg: &Diagnostic,
    file_id: &HashMap<usize, usize>,
) -> diagnostic::Diagnostic<usize> {
    let diagnostic = diagnostic::Diagnostic::new(match msg.level {
        Level::Debug => diagnostic::Severity::Help,
        Level::Info => diagnostic::Severity::Note,
        Level::Error => diagnostic::Severity::Error,
        Level::Warning => diagnostic::Severity::Warning,
    })
    .with_message(msg.message.to_owned());

    let mut labels = Vec::new();
    let mut plain_notes = Vec::new();

    if let Loc::File(file_no, start, end) = msg.loc {
        labels.push(diagnostic::Label::primary(file_id[&file_no], start..end));
    }

    for note in &msg.notes {
        if let Loc::File(file_no, start, end) = note.loc {
            labels.push(
                diagnostic::Label::secondary(file_id[&file_no], start..end)
                    .with_message(note.message.to_owned()),
            );
        } else {
            plain_notes.push(note.message.to_owned());
        }
    }

    let diagnostic = if labels.is_empty() {
        diagnostic
    } else {
        diagnostic.with_labels(labels)
    };

    if plain_notes.is_empty() {
        diagnostic
    } else {
        diagnostic.with_notes(plain_notes)
    }
}

impl Namespace {
    /// Print the diagnostics to stdout with plain formatting
    pub fn print_diagnostics_in_plain(&self, cache: &FileResolver, debug: bool) {
        let (files, file_id) = self.convert_files(cache);

        let config = term::Config::default();

        for msg in self.diagnostics.iter() {
            if msg.level == Level::Debug && !debug {
                continue;
            }

            let diagnostic = convert_diagnostic(msg, &file_id);

            let mut buffer = RawBuffer::new();

            term::emit(&mut buffer, &config, &files, &diagnostic).unwrap();

            println!("{}", buffer.into_string());
        }
    }

    /// Print the diagnostics to stderr with fancy formatting
    pub fn print_diagnostics(&self, cache: &FileResolver, debug: bool) {
        let (files, file_id) = self.convert_files(cache);

        let writer = term::termcolor::StandardStream::stderr(term::termcolor::ColorChoice::Auto);
        let config = term::Config::default();

        for msg in self.diagnostics.iter() {
            if msg.level == Level::Debug && !debug {
                continue;
            }

            let diagnostic = convert_diagnostic(msg, &file_id);

            term::emit(&mut writer.lock(), &config, &files, &diagnostic).unwrap();
        }
    }

    pub fn diagnostics_as_json(&self, cache: &FileResolver) -> Vec<OutputJson> {
        let (files, file_id) = self.convert_files(cache);
        let mut json = Vec::new();

        let config = term::Config {
            display_style: term::DisplayStyle::Short,
            ..Default::default()
        };

        for msg in self.diagnostics.iter() {
            if msg.level == Level::Info || msg.level == Level::Debug {
                continue;
            }

            let diagnostic = convert_diagnostic(msg, &file_id);

            let mut buffer = RawBuffer::new();

            term::emit(&mut buffer, &config, &files, &diagnostic).unwrap();

            let location = if let Loc::File(file_no, start, end) = msg.loc {
                Some(LocJson {
                    file: format!("{}", self.files[file_no]),
                    start: start + 1,
                    end: end + 1,
                })
            } else {
                None
            };

            json.push(OutputJson {
                sourceLocation: location,
                ty: format!("{:?}", msg.ty),
                component: "general".to_owned(),
                severity: msg.level.to_string(),
                message: msg.message.clone(),
                formattedMessage: buffer.into_string(),
            });
        }

        json
    }

    fn convert_files(
        &self,
        cache: &FileResolver,
    ) -> (files::SimpleFiles<String, Arc<str>>, HashMap<usize, usize>) {
        let mut files = files::SimpleFiles::new();
        let mut file_id = HashMap::new();

        for (file_no, file) in self.files.iter().enumerate() {
            if file.cache_no.is_some() {
                let (contents, _) = cache.get_file_contents_and_number(&file.path);
                file_id.insert(file_no, files.add(format!("{file}"), contents.to_owned()));
            }
        }

        (files, file_id)
    }
}

#[derive(Default)]
pub struct RawBuffer {
    buf: Vec<u8>,
}

impl RawBuffer {
    pub fn new() -> RawBuffer {
        RawBuffer::default()
    }

    pub fn into_string(self) -> String {
        String::from_utf8(self.buf).unwrap()
    }
}

impl io::Write for RawBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl term::termcolor::WriteColor for RawBuffer {
    fn supports_color(&self) -> bool {
        false
    }

    fn set_color(&mut self, _: &term::termcolor::ColorSpec) -> io::Result<()> {
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        Ok(())
    }
}
