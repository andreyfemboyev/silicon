; Fibonacci, seven iterations
SET r0, 0
SET r1, 1
SET r2, 7
SET r3, 1
loop:
ADD r1, r0
SUB r0, r1
SET r3, 0
SUB r3, r0
MOV r0, r3
SET r3, 1
SUB r2, r3
JNZ loop
ST r1, 0xf0
HALT
