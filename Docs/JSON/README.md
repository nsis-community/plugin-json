# Using the JSON plug-in

Read and write JSON and JSONC files from an NSIS script. Comments, spacing, key order and line endings survive a round trip, so a script can edit a config file without reformatting it.

## Quick start

```nsis
!include "JSON.nsh"                        ; only needed for ${JsonForEach}

JSON::Load "cfg" "$INSTDIR\settings.json"  ; parse once, under that name
JSON::Get "cfg" "server.port"              ; arguments are name, path
Pop $0                                     ; $0 = "80"
JSON::SetInt "cfg" "server.port" 8080
JSON::Save "cfg" "$INSTDIR\settings.json"
JSON::Free "cfg"
```

The name is any string you like, so a `!define` keeps it in one place. A `Var` works too (`StrCpy $Doc "cfg"`, then `$Doc` in each call) if the name is only known at run time. The examples below write `"cfg"` out for brevity.

To create a file from scratch, use `JSON::New "cfg"` instead of `Load`. It starts as an empty object.

## Paths

A path addresses a value inside the document:

| Path              | Means                                 |
| ----------------- | ------------------------------------- |
| `title`           | top-level key                         |
| `server.port`     | key in an object                      |
| `plugins[1].name` | key in the second element of an array |
| `[0].name`        | first element of a top-level array    |
| `env."app.mode"`  | quoted key containing a dot           |
| `""`              | the document root                     |

A bare key runs up to the next `.`, `[`, `]` or `"`, so it may contain spaces. In a quoted key, `\"` and `\\` stand for a quote and a backslash.

Setting a path that doesn't exist creates it, including parent objects. `plugins[2]` appends if the array has two elements. The empty path replaces the whole document.

## Commands

| Command                 | Arguments         | Result                                                                                                                                       |
| ----------------------- | ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `Load`                  | `name file`       | Parses a UTF-8 JSON or JSONC file. Replaces any document of that name.                                                                       |
| `New`                   | `name`            | An empty object.                                                                                                                             |
| `Save`                  | `name file`       | Writes the document as UTF-8. Keeps the BOM if the file had one.                                                                             |
| `Free`                  | `name`            | Forgets the document. Open documents are also freed when the installer exits.                                                                |
| `Get`                   | `name path`       | Pushes the value.                                                                                                                            |
| `Type`                  | `name path`       | Pushes `string`, `integer`, `float`, `boolean`, `null`, `array` or `object`. A missing path is an error, so this doubles as "has key".       |
| `Count`                 | `name path`       | Pushes the array length or the number of object keys.                                                                                        |
| `EntryAt`               | `name path index` | Pushes the value, then the key (for an array, the index). `Pop` the key first.                                                               |
| `SetString`             | `name path value` | A string. The plug-in does the quoting.                                                                                                      |
| `SetInt`                | `name path value` | A 64-bit integer.                                                                                                                            |
| `SetFloat`              | `name path value` | A finite float: `3.14`, `1e6`.                                                                                                               |
| `SetBool`               | `name path value` | `1`/`true` or `0`/`false`.                                                                                                                   |
| `SetNull`               | `name path`       | `null`.                                                                                                                                      |
| `SetRaw`                | `name path value` | Any value in JSON syntax: `[1, 2]`, `{"a": "x"}`, `12345678901234567890`.                                                                    |
| `SetArray`, `SetObject` | `name path`       | An empty array or object.                                                                                                                    |
| `Remove`                | `name path`       | Removes a key or array element.                                                                                                              |
| `LastError`             |                   | Pushes the message from the last failure.                                                                                                    |

## Error handling

Every failure sets the NSIS error flag and pushes nothing. Clear the flag first, then check it:

```nsis
ClearErrors

JSON::Load "cfg" "$INSTDIR\settings.json"

${If} ${Errors}
	JSON::LastError
	Pop $0
	MessageBox MB_OK "Could not read settings: $0"
	Abort
${EndIf}
```

The one exception is a value longer than the installer's `NSIS_MAX_STRLEN`: it is pushed truncated and the flag is set as well.

A failed write leaves the document as it was.

## Recipes

**Check whether a key exists**

```nsis
ClearErrors

JSON::Type "cfg" "server.tls"

${IfNot} ${Errors}
	Pop $0   ; the type name
${EndIf}
```

**Loop over an object or array** (`JSON.nsh`)

```nsis
${JsonForEach} "cfg" "server" $0 $1
	DetailPrint "$0 = $1"
${JsonNext}
```

Each pass sets the key and the value; for an array the key is the index. `${JsonBreak}` leaves the loop and `${Continue}` skips to the next entry. A path that doesn't exist, or isn't an array or object, runs zero passes. Loops nest.

**Write a container in one call**

```nsis
JSON::SetRaw "cfg" "server.ports" "[80, 443]"
JSON::SetRaw "cfg" "server.limits" '{"cpu": 2, "mem": "1G"}'
```

**Write an integer beyond 64 bits**

```nsis
JSON::SetRaw "cfg" "id" "12345678901234567890"
```

## Reading: what `Get` returns

| JSON value                      | `Type` reports | `Get` returns                         |
| ------------------------------- | -------------- | ------------------------------------- |
| string                          | `string`       | the decoded value                     |
| number without fraction or exponent | `integer`  | the number exactly as written         |
| other number                    | `float`        | the number exactly as written         |
| `true` / `false`                | `boolean`      | `true` / `false`                      |
| `null`                          | `null`         | `null`                                |
| array                           | `array`        | compact JSON                          |
| object                          | `object`       | compact JSON                          |

See `type_name` and `render` in [doc.rs](../../Contrib/JSON/src/doc.rs).

## Caveats

- **Comments inside a value written with `SetRaw` are lost.** Only the value you pass is affected, not the rest of the file.
- **`Get` on a container returns compact JSON, and a string comes back decoded.** Passing an array or object result to `SetRaw` works. Passing a plain string result fails, because it isn't valid JSON. Use `SetString` for strings.
- **A string containing `\u0000` gets cut off at the NUL.** NSIS strings are NUL-terminated, so the script sees only the part before the NUL. No error is reported.
- **`SetInt` uses NSIS syntax.** `0x1F` is hex, `0755` is octal, and garbage becomes `0`. Use `SetRaw` for anything else, including integers beyond 64 bits.
- **`SetFloat` takes finite numbers only.** JSON has no `inf` or `nan`.
- **NSIS integer math is 32-bit.** `Get` returns 64-bit integers exactly as strings, but `IntOp` and `IntCmp` wrap values above 2³¹−1.
- **Value length is capped by `NSIS_MAX_STRLEN`**, 1024 in a stock makensis build.
