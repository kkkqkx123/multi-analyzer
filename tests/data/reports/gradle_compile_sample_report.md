# Gradle Compile (Sample) Analysis Report

**Command**: `gradle compileJava (sample output)`

## Summary

- **Total Issues**: 12
- **Errors**: 10
- **Warnings**: 2
- **Info**: 0
- **Files with Issues**: 5

## Issue Details (Grouped by File)

### /Users/user/project/src/main/java/com/example/Broken.java

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 8 | 0 | Error | cannot find symbol |
| 8 | 0 | Error | cannot find symbol |
| 8 | 0 | Error | cannot find symbol |
| 12 | 0 | Error | cannot find symbol |
| 12 | 0 | Error | cannot find symbol |
| 12 | 0 | Error | cannot find symbol |
| 5 | 0 | Error | class Broken is public, should be declared in a file named Broken.java |

### /Users/user/project/src/main/java/com/example/App.java

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 15 | 0 | Warning | [unchecked] unchecked conversion |

### /Users/user/project/src/main/kotlin/App.kt

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 10 | 0 | Error | unresolved reference: undefinedFunction |

### build.gradle

| Line | Column | Level | Message |
|------|--------|-------|---------|
| - | - | Error | > Task :compileJava FAILED |
| - | - | Error | BUILD FAILED in 2s |

### /Users/user/project/src/main/java/com/example/Utils.java

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 22 | 0 | Warning | [deprecation] getYear() in Date has been deprecated |

## Raw Output

View raw command output: [samples/gradle_compile_sample.txt](samples/gradle_compile_sample.txt)

