# Maven Compile Analysis Report

**Command**: `mvn compile`

## Summary

- **Total Issues**: 9
- **Errors**: 4
- **Warnings**: 5
- **Info**: 0
- **Files with Issues**: 4

## Issue Details (Grouped by File)

### D:\project\test\fixtures\maven-project\src\main\java\com\example\App.java

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 18 | 9 | Warning | [unchecked] unchecked conversion |

### D:\project\test\fixtures\maven-project\src\main\java\com\example\Broken.java

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 5 | 1 | Error | class Broken is public, should be declared in a file named Broken.java |
| 8 | 9 | Error | cannot find symbol |
| 12 | 16 | Error | cannot find symbol |

### pom.xml

| Line | Column | Level | Message |
|------|--------|-------|---------|
| - | - | Error | Failed to execute goal org.apache.maven.plugins:maven-compiler-plugin:3.10.1:compile (default-compile) on project maven-test-project: Compilation failure: Compilation failure: |

### D:\project\test\fixtures\maven-project\src\main\java\com\example\Utils.java

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 12 | 43 | Warning | [unchecked] unchecked conversion |
| 19 | 26 | Warning | [deprecation] getYear() in Date has been deprecated |
| 20 | 27 | Warning | [deprecation] getMonth() in Date has been deprecated |
| 21 | 25 | Warning | [deprecation] getDate() in Date has been deprecated |

## Raw Output

View raw command output: [samples/maven_compile_sample.txt](samples/maven_compile_sample.txt)

