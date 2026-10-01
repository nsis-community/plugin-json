; Smoke test for the JSON plug-in.
;
; Built by `mise run smoke`, which supplies PLUGINDIR and OUTFILE and picks
; the variant with -XTarget. Results go to smoke.log next to the installer;
; the installer itself is silent.

Unicode true

!include "LogicLib.nsh"
!include "${__FILEDIR__}/../Include/JSON.nsh"

!ifndef PLUGINDIR
	!error "define PLUGINDIR: the Plugins/<variant> directory holding JSON.dll"
!endif
!ifndef OUTFILE
	!define OUTFILE "smoke.exe"
!endif

!addplugindir "${PLUGINDIR}"

Name "JSON plug-in smoke test"
OutFile "${OUTFILE}"
RequestExecutionLevel user
SilentInstall silent
ShowInstDetails nevershow

Var LOG
Var FAILURES
Var FILE

; Backticks, because JSON values hold double quotes.
!macro Expect name expected
	StrCpy $9 `${expected}`
	${If} $0 == $9
		FileWrite $LOG "ok   ${name}: $0$\r$\n"
	${Else}
		IntOp $FAILURES $FAILURES + 1
		FileWrite $LOG "FAIL ${name}: expected '$9', got '$0'$\r$\n"
	${EndIf}
!macroend

!macro ExpectError name
	${If} ${Errors}
		JSON::LastError
		Pop $0
		FileWrite $LOG "ok   ${name}: error flag set ($0)$\r$\n"
	${Else}
		IntOp $FAILURES $FAILURES + 1
		FileWrite $LOG "FAIL ${name}: error flag not set$\r$\n"
	${EndIf}
!macroend

!macro ExpectNoError name
	${If} ${Errors}
		IntOp $FAILURES $FAILURES + 1
		JSON::LastError
		Pop $0
		FileWrite $LOG "FAIL ${name}: error flag set ($0)$\r$\n"
	${Else}
		FileWrite $LOG "ok   ${name}: no error$\r$\n"
	${EndIf}
!macroend

Section "Smoke"
	StrCpy $FAILURES 0
	FileOpen $LOG "$EXEDIR\smoke.log" w
	InitPluginsDir

	FileOpen $FILE "$PLUGINSDIR\in.json" w
	FileWrite $FILE "{$\r$\n"
	FileWrite $FILE "  // comment$\r$\n"
	FileWrite $FILE "  $\"title$\": $\"smoke$\",$\r$\n"
	FileWrite $FILE "  $\"server$\": {$\r$\n"
	FileWrite $FILE "    $\"host$\": $\"localhost$\", // keep$\r$\n"
	FileWrite $FILE "    $\"port$\": 80$\r$\n"
	FileWrite $FILE "  },$\r$\n"
	FileWrite $FILE "  $\"plugins$\": [{$\"name$\": $\"a$\"}, {$\"name$\": $\"b$\"}],$\r$\n"
	FileWrite $FILE "  $\"id$\": 12345678901234567890$\r$\n"
	FileWrite $FILE "}$\r$\n"
	FileClose $FILE

	; Reading.
	ClearErrors
	JSON::Load "cfg" "$PLUGINSDIR\in.json"
	!insertmacro ExpectNoError "Load"

	JSON::Get "cfg" "title"
	Pop $0
	!insertmacro Expect "Get/string" "smoke"

	JSON::Get "cfg" "server.port"
	Pop $0
	!insertmacro Expect "Get/integer" "80"

	JSON::Get "cfg" "id"
	Pop $0
	!insertmacro Expect "Get/big integer" "12345678901234567890"

	JSON::Get "cfg" "plugins[1].name"
	Pop $0
	!insertmacro Expect "Get/index" "b"

	JSON::Get "cfg" "plugins[0]"
	Pop $0
	!insertmacro Expect "Get/object" '{"name":"a"}'

	JSON::Type "cfg" "server"
	Pop $0
	!insertmacro Expect "Type" "object"

	JSON::Count "cfg" "plugins"
	Pop $0
	!insertmacro Expect "Count" "2"

	; Argument order and push order through the real stack.
	JSON::EntryAt "cfg" "server" 1
	Pop $1
	Pop $2
	StrCpy $0 "$1=$2"
	!insertmacro Expect "EntryAt" "port=80"

	; Iteration.
	StrCpy $0 ""
	${JsonForEach} "cfg" "server" $1 $2
		StrCpy $0 "$0$1=$2;"
	${JsonNext}
	!insertmacro Expect "ForEach" "host=localhost;port=80;"

	StrCpy $0 ""
	${JsonForEach} "cfg" "plugins" $1 $2
		${JsonForEach} "cfg" "plugins[$1]" $3 $4
			StrCpy $0 "$0$1.$3=$4;"
		${JsonNext}
	${JsonNext}
	!insertmacro Expect "ForEach/nested" "0.name=a;1.name=b;"

	StrCpy $0 ""
	${JsonForEach} "cfg" "" $1 $2
		StrCpy $0 "$0$1;"
		${If} $1 == "server"
			${JsonBreak}
		${EndIf}
	${JsonNext}
	!insertmacro Expect "ForEach/break" "title;server;"

	StrCpy $0 "untouched"
	${JsonForEach} "cfg" "missing" $1 $2
		StrCpy $0 "ran"
	${JsonNext}
	!insertmacro Expect "ForEach/missing" "untouched"

	; Writing, then reading the file back.
	ClearErrors
	JSON::SetInt "cfg" "server.port" "0x1F90"
	JSON::SetString "cfg" "server.host" "example.com"
	JSON::SetBool "cfg" "server.tls" 1
	JSON::SetNull "cfg" "server.proxy"
	JSON::SetString "cfg" "plugins[2].name" "c"
	JSON::SetString "cfg" "new.nested.key" "created"
	JSON::SetRaw "cfg" "raw" '[1, {"a": true}]'
	JSON::Save "cfg" "$PLUGINSDIR\out.json"
	JSON::Free "cfg"
	JSON::Load "cfg" "$PLUGINSDIR\out.json"
	!insertmacro ExpectNoError "Set/Save/Load"

	JSON::Get "cfg" "server.port"
	Pop $0
	!insertmacro Expect "SetInt" "8080"
	JSON::Get "cfg" "server.tls"
	Pop $0
	!insertmacro Expect "SetBool" "true"
	JSON::Type "cfg" "server.proxy"
	Pop $0
	!insertmacro Expect "SetNull" "null"
	JSON::Get "cfg" "plugins[2].name"
	Pop $0
	!insertmacro Expect "Set/append" "c"
	JSON::Get "cfg" "new.nested.key"
	Pop $0
	!insertmacro Expect "Set/create" "created"
	JSON::Get "cfg" "raw"
	Pop $0
	!insertmacro Expect "SetRaw" '[1,{"a":true}]'

	; The comment and the CRLF line endings survive.
	FileOpen $FILE "$PLUGINSDIR\out.json" r
	FileRead $FILE $0
	FileRead $FILE $0
	FileRead $FILE $0
	FileRead $FILE $0
	FileRead $FILE $0
	FileClose $FILE
	!insertmacro Expect "Save/fidelity" "    $\"host$\": $\"example.com$\", // keep$\r$\n"

	; Failures set the flag and push nothing.
	Push "sentinel"
	ClearErrors
	JSON::Get "cfg" "no.such.key"
	!insertmacro ExpectError "Get/missing"
	ClearErrors
	JSON::Load "other" "$PLUGINSDIR\does-not-exist.json"
	!insertmacro ExpectError "Load/missing"
	ClearErrors
	JSON::SetBool "cfg" "x" "maybe"
	!insertmacro ExpectError "SetBool/invalid"
	Pop $0
	!insertmacro Expect "Failure/stack" "sentinel"

	; The longest value this installer can hold round-trips. Against a stock
	; makensis that is 1023 characters, against /DNSIS_MAX_STRLEN=8192 it is
	; 8191, with the same DLL.
	!define /math MAX ${NSIS_MAX_STRLEN} - 1
	StrCpy $1 ""
	StrCpy $2 0
	${Do}
		StrCpy $1 "$1abcdefghij"
		IntOp $2 $2 + 10
	${LoopUntil} $2 >= ${MAX}
	StrCpy $1 $1 ${MAX}
	ClearErrors
	JSON::SetString "cfg" "long" $1
	JSON::Get "cfg" "long"
	Pop $0
	!insertmacro ExpectNoError "Get/max"
	StrLen $3 $0
	StrCpy $0 $3
	!insertmacro Expect "Get/max length" "${MAX}"

	; One character more than that must come back truncated, with the flag.
	FileOpen $FILE "$PLUGINSDIR\long.json" w
	; In pieces: the whole line would not fit in an NSIS string either.
	FileWrite $FILE "{$\"longer$\": $\"x"
	FileWrite $FILE $1
	FileWrite $FILE "$\"}$\n"
	FileClose $FILE
	JSON::Load "cfg" "$PLUGINSDIR\long.json"
	ClearErrors
	JSON::Get "cfg" "longer"
	Pop $4
	!insertmacro ExpectError "Get/truncated"
	StrLen $0 $4
	!insertmacro Expect "Get/truncated length" "${MAX}"

	JSON::Free "cfg"
	FileWrite $LOG "note NSIS_MAX_STRLEN is ${NSIS_MAX_STRLEN}$\r$\n"

	${If} $FAILURES == 0
		FileWrite $LOG "ALL PASSED$\r$\n"
	${Else}
		FileWrite $LOG "$FAILURES FAILED$\r$\n"
	${EndIf}
	FileClose $LOG

	SetErrorLevel $FAILURES
SectionEnd
