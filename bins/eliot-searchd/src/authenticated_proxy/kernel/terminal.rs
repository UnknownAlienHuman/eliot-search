//! Closed child response terminal classes.

use super::exchange::event_name;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Terminal {
    Single,
    DirectoryIndex,
    StreamingSearch,
    SearchPage,
    SourceList,
    Shutdown,
}

impl Terminal {
    pub(super) fn for_command(command: &str) -> Result<Self, String> {
        let name = command
            .split('\t')
            .next()
            .ok_or_else(|| "LOOPBACK_DIRECT_COMMAND_INVALID".to_owned())?;
        Ok(match name {
            "index-directory" => Self::DirectoryIndex,
            "search" => Self::StreamingSearch,
            "search-page" | "continue" => Self::SearchPage,
            "list-sources" => Self::SourceList,
            "shutdown" => Self::Shutdown,
            _ => Self::Single,
        })
    }

    /// Exact event for the live, bodyless diagnostic commands. The transport
    /// still uses the existing Single boundary, but any event is not a valid
    /// reply to a diagnostic. Invalid command/terminal pairs fail before stdin.
    pub(super) fn diagnostic_event(self, command: &str) -> Result<Option<&'static str>, String> {
        let name = command.split('\t').next().unwrap_or_default();
        let event = match name {
            "health" => "health",
            "version" => "version",
            "status" => "provider_status",
            _ => return Ok(None),
        };
        if command != name || self != Self::Single {
            return Err("LOOPBACK_DIRECT_COMMAND_INVALID".to_owned());
        }
        Ok(Some(event))
    }

    pub(super) fn reached(self, line: &str) -> bool {
        let event = event_name(line);
        let expected = match self {
            Self::Single => return event.is_some(),
            Self::DirectoryIndex => "directory_index_complete",
            Self::StreamingSearch => "corpus_search_complete",
            Self::SearchPage => "search_page_complete",
            Self::SourceList => "source_list_complete",
            Self::Shutdown => "data_root_stopped",
        };
        event == Some(expected)
    }
}
