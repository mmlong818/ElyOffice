; Shared validation for installer and uninstaller ownership manifests.
; Relative Win32 paths only. A trailing separator marks a directory entry.
Var ElyPathInput
Var ElyPathNormalized
Var ElyPathSegment
Var ElyPathBase
Var ElyPathChar
Var ElyPathSegmentChar
Var ElyPathLen
Var ElyPathSegmentLen
Var ElyPathI
Var ElyPathSegmentI
Var ElyPathIsDir
Var ElyPathBad

!macro ElyManifestFunctions UN
Function ${UN}ElyNext
next:
  ClearErrors
  FileRead $R2 $R3
  IfErrors 0 trim
    StrCpy $R6 ""
    Return
trim:
  StrCpy $ElyPathChar $R3 1 -1
  StrCmp $ElyPathChar "$\r" cut
  StrCmp $ElyPathChar "$\n" cut
  StrCmp $R3 "" next
  Call ${UN}ElyValidatePath
  Return
cut:
  StrCpy $R3 $R3 -1
  Goto trim
FunctionEnd

Function ${UN}ElyValidateSegment
  StrCpy $ElyPathBad "1"
  StrCmp $ElyPathSegment "" segment_done
  StrCmp $ElyPathSegment "." segment_done
  StrCmp $ElyPathSegment ".." segment_done
  StrCpy $ElyPathSegmentChar $ElyPathSegment 1 -1
  StrCmp $ElyPathSegmentChar "." segment_done
  StrCmp $ElyPathSegmentChar " " segment_done
  StrCpy $ElyPathBase ""
  StrLen $ElyPathSegmentLen $ElyPathSegment
  StrCpy $ElyPathSegmentI 0
segment_base:
  IntCmp $ElyPathSegmentI $ElyPathSegmentLen segment_reserved segment_base_char segment_done
segment_base_char:
  StrCpy $ElyPathSegmentChar $ElyPathSegment 1 $ElyPathSegmentI
  StrCmp $ElyPathSegmentChar "." segment_reserved
  StrCpy $ElyPathBase "$ElyPathBase$ElyPathSegmentChar"
  IntOp $ElyPathSegmentI $ElyPathSegmentI + 1
  Goto segment_base
segment_reserved:
  StrCmp $ElyPathBase "CON" segment_done
  StrCmp $ElyPathBase "PRN" segment_done
  StrCmp $ElyPathBase "AUX" segment_done
  StrCmp $ElyPathBase "NUL" segment_done
  StrCmp $ElyPathBase "CLOCK$$" segment_done
  StrCmp $ElyPathBase "CONIN$$" segment_done
  StrCmp $ElyPathBase "CONOUT$$" segment_done
  StrCmp $ElyPathBase "COM1" segment_done
  StrCmp $ElyPathBase "COM2" segment_done
  StrCmp $ElyPathBase "COM3" segment_done
  StrCmp $ElyPathBase "COM4" segment_done
  StrCmp $ElyPathBase "COM5" segment_done
  StrCmp $ElyPathBase "COM6" segment_done
  StrCmp $ElyPathBase "COM7" segment_done
  StrCmp $ElyPathBase "COM8" segment_done
  StrCmp $ElyPathBase "COM9" segment_done
  StrCmp $ElyPathBase "LPT1" segment_done
  StrCmp $ElyPathBase "LPT2" segment_done
  StrCmp $ElyPathBase "LPT3" segment_done
  StrCmp $ElyPathBase "LPT4" segment_done
  StrCmp $ElyPathBase "LPT5" segment_done
  StrCmp $ElyPathBase "LPT6" segment_done
  StrCmp $ElyPathBase "LPT7" segment_done
  StrCmp $ElyPathBase "LPT8" segment_done
  StrCmp $ElyPathBase "LPT9" segment_done
  StrCmp $ElyPathBase "COM¹" segment_done
  StrCmp $ElyPathBase "COM²" segment_done
  StrCmp $ElyPathBase "COM³" segment_done
  StrCmp $ElyPathBase "LPT¹" segment_done
  StrCmp $ElyPathBase "LPT²" segment_done
  StrCmp $ElyPathBase "LPT³" segment_done
  StrCpy $ElyPathBad "0"
segment_done:
FunctionEnd

Function ${UN}ElyValidatePath
  Push $0
  Push $1
  StrCpy $R6 "x"
  StrCpy $ElyPathInput $R3
  StrCpy $ElyPathNormalized ""
  StrLen $ElyPathLen $ElyPathInput
  StrCmp $ElyPathLen 0 path_invalid
  StrCpy $ElyPathI 0
path_normalize:
  IntCmp $ElyPathI $ElyPathLen path_normalized path_normalize_char path_invalid
path_normalize_char:
  StrCpy $ElyPathChar $ElyPathInput 1 $ElyPathI
  StrCmp $ElyPathChar "/" 0 +2
    StrCpy $ElyPathChar "\"
  StrCmp $ElyPathChar "<" path_invalid
  StrCmp $ElyPathChar ">" path_invalid
  StrCmp $ElyPathChar ":" path_invalid
  StrCmp $ElyPathChar '"' path_invalid
  StrCmp $ElyPathChar "|" path_invalid
  StrCmp $ElyPathChar "?" path_invalid
  StrCmp $ElyPathChar "*" path_invalid
  System::Call 'kernel32::GetStringTypeW(i 1, w "$ElyPathChar", i 1, *i .r0) i .r1'
  StrCmp $1 0 path_invalid
  IntOp $0 $0 & 0x20
  StrCmp $0 0 path_char_ok
  Goto path_invalid
path_char_ok:
  StrCpy $ElyPathNormalized "$ElyPathNormalized$ElyPathChar"
  IntOp $ElyPathI $ElyPathI + 1
  Goto path_normalize
path_normalized:
  StrCpy $R3 $ElyPathNormalized
  StrCpy $ElyPathChar $R3 1
  StrCmp $ElyPathChar "\" path_invalid
  StrCpy $ElyPathIsDir "f"
  StrCpy $ElyPathChar $R3 1 -1
  StrCmp $ElyPathChar "\" 0 path_segments
  StrCpy $ElyPathIsDir "d"
  StrCpy $R3 $R3 -1
  StrCmp $R3 "" path_invalid
path_segments:
  StrCpy $ElyPathSegment ""
  StrLen $ElyPathLen $R3
  StrCpy $ElyPathI 0
path_segment_loop:
  IntCmp $ElyPathI $ElyPathLen path_segment_final path_segment_char path_invalid
path_segment_char:
  StrCpy $ElyPathChar $R3 1 $ElyPathI
  StrCmp $ElyPathChar "\" path_segment_end
  StrCpy $ElyPathSegment "$ElyPathSegment$ElyPathChar"
  IntOp $ElyPathI $ElyPathI + 1
  Goto path_segment_loop
path_segment_end:
  Call ${UN}ElyValidateSegment
  StrCmp $ElyPathBad "1" path_invalid
  StrCpy $ElyPathSegment ""
  IntOp $ElyPathI $ElyPathI + 1
  Goto path_segment_loop
path_segment_final:
  Call ${UN}ElyValidateSegment
  StrCmp $ElyPathBad "1" path_invalid
  StrCpy $R6 $ElyPathIsDir
  Pop $1
  Pop $0
  Return
path_invalid:
  StrCpy $R6 "x"
  Pop $1
  Pop $0
FunctionEnd

; $R0: list path. Returns $R5 empty for a valid full list, otherwise its path.
Function ${UN}ElyPreflightList
  StrCpy $R5 ""
  ClearErrors
  FileOpen $R2 "$R0" r
  IfErrors preflight_bad
preflight_next:
  Call ${UN}ElyNext
  StrCmp $R6 "" preflight_done
  StrCmp $R6 "x" preflight_bad_open
  Goto preflight_next
preflight_done:
  FileClose $R2
  Return
preflight_bad_open:
  FileClose $R2
preflight_bad:
  StrCpy $R5 "$R0"
FunctionEnd

; $R9: backup directory. Refuse to traverse a pre-existing reparse point.
Function ${UN}ElyOldSafe
  Push $0
  Push $1
  StrCpy $R5 ""
  System::Call 'kernel32::GetFileAttributesW(w "$R9") i .r0'
  IntCmp $0 -1 old_missing old_present old_present
old_missing:
  IfFileExists "$R9" 0 old_safe
  Goto old_unsafe
old_present:
  IntOp $1 $0 & 0x400
  IntCmp $1 0 old_safe old_unsafe old_unsafe
old_unsafe:
  StrCpy $R5 "$R9"
old_safe:
  Pop $1
  Pop $0
FunctionEnd
!macroend

!insertmacro ElyManifestFunctions ""
!insertmacro ElyManifestFunctions "un."
