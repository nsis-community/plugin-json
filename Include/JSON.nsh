; JSON.nsh — loops over JSON arrays and objects.
;
;   ${JsonForEach} "cfg" "server" $0 $1
;       DetailPrint "$0 = $1"
;   ${JsonNext}
;
; Each pass sets the key and the value; for an array the key is the index.
; ${JsonBreak} leaves the loop and ${Continue} skips to the next entry. A
; path that does not exist, or is not an array or object, runs zero passes.
;
; Loops nest: each one keeps its counter in its own Var /GLOBAL, named from
; ${__COUNTER__}, and a define stack (the pattern of LogicLib's _PushScope)
; tracks which loop is innermost.

!ifndef JSON_NSH
!define JSON_NSH

!include "LogicLib.nsh"

!macro _JsonPushScope id
	!ifdef _JsonId
		!define _JsonPrev${id} ${_JsonId}
		!undef _JsonId
	!endif
	!define _JsonId ${id}
!macroend

!macro _JsonPopScope
	!ifndef _JsonId
		!error "${JsonNext} without a ${JsonForEach}"
	!endif
	!ifdef _JsonPrev${_JsonId}
		!define _JsonCur ${_JsonId}
		!undef _JsonId
		!define _JsonId ${_JsonPrev${_JsonCur}}
		!undef _JsonPrev${_JsonCur}
		!undef _JsonCur
	!else
		!undef _JsonId
	!endif
!macroend

!macro JsonForEach name path key value
	!insertmacro _JsonPushScope ${__COUNTER__}
	Var /GLOBAL _JsonI${_JsonId}
	Var /GLOBAL _JsonN${_JsonId}
	StrCpy $_JsonN${_JsonId} 0
	ClearErrors
	JSON::Count "${name}" "${path}"
	${IfNot} ${Errors}
		Pop $_JsonN${_JsonId}
	${EndIf}
	StrCpy $_JsonI${_JsonId} 0
	${DoWhile} $_JsonI${_JsonId} < $_JsonN${_JsonId}
		JSON::EntryAt "${name}" "${path}" $_JsonI${_JsonId}
		Pop ${key}
		Pop ${value}
		IntOp $_JsonI${_JsonId} $_JsonI${_JsonId} + 1
!macroend
!define JsonForEach "!insertmacro JsonForEach"

!macro JsonNext
	${Loop}
	!insertmacro _JsonPopScope
!macroend
!define JsonNext "!insertmacro JsonNext"

!define JsonBreak "${ExitDo}"

!endif
