# Owner-fenced DIRECT stdio service

`eliot-searchd --serve-data-root ROOT` acquires the operating-system data-root
lock, opens and verifies the persistent DIRECT store, then serves commands until
`shutdown` or EOF. One process owns the root for the entire session.

This is the current local development transport. It is deliberately not called
the production authenticated endpoint.

## Framing

- one UTF-8 command per line;
- maximum command line: 256 KiB;
- fields are separated by a single tab;
- queries and native path bytes are hexadecimal;
- one newline-delimited JSON object per response record;
- maximum response record: 64 KiB;
- a command failure emits one `event=error` record and leaves the session open;
- EOF performs an orderly owner release.

Path encoding:

- Windows: little-endian UTF-16 code units;
- Unix: native `OsStr` bytes;
- other targets: UTF-8.

The human-facing CLI bridge performs this encoding automatically:

```powershell
./target/debug/eliot-search.exe serve-data-root .\.eliot-search-data
```

It accepts:

```text
health
version
verify
list-sources
index-file PATH
index-directory PATH
search QUERY
search-i QUERY
retire SOURCE_ID
read-revision REVISION_ID START END
gc-dry-run
gc-apply
shutdown
```

## Wire commands

```text
health
version
verify
list-sources
index-file<TAB>PATH_HEX
index-directory<TAB>PATH_HEX
search<TAB>sensitive<TAB>QUERY_UTF8_HEX
search<TAB>ascii-insensitive<TAB>QUERY_UTF8_HEX
retire<TAB>SOURCE_ID
read-revision<TAB>REVISION_ID<TAB>START<TAB>END
gc<TAB>dry-run
gc<TAB>apply
shutdown
```

Multi-record terminal events:

| Command | Terminal event |
|---|---|
| `index-directory` | `directory_index_complete` |
| `search` | `corpus_search_complete` |
| `list-sources` | `source_list_complete` |
| `shutdown` | `data_root_stopped` |

All other commands return one response record. `event=error` terminates only the
current command.

## Current evidence boundary

Persistent search results are source-backed because the daemon verifies:

1. the complete SHA-256 source-event chain;
2. immutable revision object type and exact byte length;
3. revision content digest;
4. deterministic revision identity;
5. exact match byte range and coordinates.

The current development revision objects are plaintext. Responses therefore
report:

```json
{"source_backed":true,"encrypted_at_rest":false}
```

Promotion to the production security profile still requires qualified key
storage, authenticated encryption, the durable production control backend, and
an authenticated local IPC endpoint.
