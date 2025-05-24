## Changelog

### 0.4.3
* Fixed `Clipboard` `fetch_text` and `set_text` not compiling on targets without
clipboard support. Now returns `ClipboardError::ClipboardNotSupported`.

### 0.4.0
* Improved performance. Text input layouts should only be regenerated after edits now. 
* The `ActiveTextInput` resource is removed. Use `InputFocus` to set the active text input.
* Fixed command binds so that they work when capslock is on.
* `TextInputNode`s are unfocused when despawned.
* Selections are cleared when a `TextInputNode` is unfocused.
* Added clipboard support for wasm32.
* Double-click to select a word.
* Triple-click to select a paragraph.
co-authored by [databasedav](https://github.com/databasedav)

### 0.3.0
* Bevy 0.16 support.

### 0.2.0
* New `line_height` parameter on `TextInputNode`. 