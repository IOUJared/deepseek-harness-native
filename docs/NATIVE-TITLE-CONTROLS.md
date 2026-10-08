# Native title-control presentation

`session/title` and `session/title-llm-request` are UI metadata, not chat messages. The native [presentation filter](<../app/src/transcript_presentation.rs>) excludes those exact retained event types from both Conversation and expanded Session records. User/assistant text mentioning those names is still displayed. Unknown event types, cancellation, warnings and Send gates remain unchanged. Raw retained records and export data are not deleted.

The [native UI](<../app/src/ui.rs>) uses admitted title metadata to update the conversation heading, sidebar name, navigation search and secondary conversation label. Title-LLM request payloads never become display titles and cause no native model request. A title snapshot supplies the baseline; a successfully admitted newer live title supplies the update. Ignored replay, stale generations, sequence gaps and malformed title values do not publish a new title.

Display titles retain at most 1,024 UTF-8 bytes per session and 4,096 session entries; raw source is preserved separately. A newer roster projection takes precedence over an older local title. Removing a session retires its local title. No clipboard task, Host operation, credential access, browser or desktop setting is involved.

Qualification and distribution scope are recorded separately from the [development archive](<NATIVE-DEVELOPMENT-PACKAGE.md>); older frozen binaries do not acquire this behavior automatically. Physical input, live title-model generation and full parity are not qualified by read-only native projection fixtures.
