# Maven Compile Analysis Report

**Command**: `mvn compile`

## Summary

- **Total Issues**: 23
- **Errors**: 8
- **Warnings**: 15
- **Info**: 0
- **Files with Issues**: 4

## Issue Details (Grouped by File)

### D:\project\test\fixtures\maven-project\src\main\java\com\example\Broken.java

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 5 | 1 | Error | class Broken is public, should be declared in a file named Broken.java |
| 8 | 9 | Error | cannot find symbol |
| 12 | 16 | Error | cannot find symbol |
| 5 | 1 | Error | class Broken is public, should be declared in a file named Broken.java |
| 8 | 9 | Error | cannot find symbol |
| 12 | 16 | Error | cannot find symbol |

### D:\project\test\fixtures\maven-project\src\main\java\com\example\App.java

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 18 | 9 | Warning | [unchecked] unchecked conversion |

### pom.xml

| Line | Column | Level | Message |
|------|--------|-------|---------|
| - | - | Warning | List rawList = new ArrayList(); |
| - | - | Warning | ^ |
| - | - | Warning | private static java.util.List rawList = new java.util.ArrayList(); |
| - | - | Warning | ^ |
| - | - | Warning | int year = date.getYear(); |
| - | - | Warning | ^ |
| - | - | Warning | int month = date.getMonth(); |
| - | - | Warning | ^ |
| - | - | Warning | int day = date.getDate(); |
| - | - | Warning | ^ |
| - | - | Error | Failed to execute goal org.apache.maven.plugins:maven-compiler-plugin:3.10.1:compile (default-compile) on project maven-test-project: Compilation failure: Compilation failure: |
| - | - | Error | -> [Help 1] |

### D:\project\test\fixtures\maven-project\src\main\java\com\example\Utils.java

| Line | Column | Level | Message |
|------|--------|-------|---------|
| 12 | 43 | Warning | [unchecked] unchecked conversion |
| 19 | 26 | Warning | [deprecation] getYear() in Date has been deprecated |
| 20 | 27 | Warning | [deprecation] getMonth() in Date has been deprecated |
| 21 | 25 | Warning | [deprecation] getDate() in Date has been deprecated |

## Raw Output

View raw command output: [samples/maven_compile_sample.txt](samples/maven_compile_sample.txt)

