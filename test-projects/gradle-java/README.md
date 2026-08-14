# gradle-java test project

Gradle + JUnit 5 test project used to verify the multi-analyzer Gradle support
(see `multi-analyzer/skills/analyzer-usage/`).

## Structure

```
gradle-java/
├── settings.gradle              # rootProject.name = 'gradle-demo'
├── build.gradle                 # Java plugin, JUnit 5, per-test event logging
└── src/
    ├── main/java/com/example/
    │   ├── App.java             # intentional unchecked-conversion warnings
    │   └── Utils.java           # intentional deprecation warnings
    ├── broken/java/com/example/
    │   └── Broken.java          # deliberate compile errors (included via -Pbroken)
    └── test/java/com/example/
        ├── AppTest.java         # 4 passing tests
        └── UtilsTest.java       # 3 passing + 1 deliberate failure
```

The project intentionally contains warnings, a failing test, and (under the
`-Pbroken` flag) compile errors so the analyzer has real issues to detect.
`testLogging` emits per-test event lines (`AppTest > testGetName PASSED`,
`UtilsTest > testFormatDate FAILED`) which the analyzer parses.

## Commands

```bash
gradle compileJava --quiet  # succeeds; emits deprecation warnings
gradle -Pbroken compileJava # fails: Broken.java compile errors
gradle test                  # fails: UtilsTest.testFormatDate failure
```

## Analyzer Usage

```bash
analyzer gradle "compileJava --quiet"
analyzer gradle "-Pbroken compileJava"
analyzer gradle "test"
analyzer run "gradle test"
analyzer rewrite "gradle compileJava"
```
