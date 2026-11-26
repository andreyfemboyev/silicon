; Sort three 32-bit words using compare and swap
SET r0, 8
SET r1, 3
SET r2, 5
CMP r0, r1
JC swap01
JMP compare12
swap01: MOV r3, r0
MOV r0, r1
MOV r1, r3
compare12: CMP r1, r2
JC swap12
JMP compare01
swap12: MOV r3, r1
MOV r1, r2
MOV r2, r3
compare01: CMP r0, r1
JC swapfinal
JMP save
swapfinal: MOV r3, r0
MOV r0, r1
MOV r1, r3
save: ST r0, 0x100
ST r1, 0x104
ST r2, 0x108
HALT
