; Subroutine and memory-mapped console
.equ console, 0x3ffc
SET r0, 65
CALL emit
SET r0, 10
CALL emit
HALT
emit:
PUSH r0
ST r0, console
POP r0
RET
