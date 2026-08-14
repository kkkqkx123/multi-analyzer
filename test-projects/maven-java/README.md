# maven-java test project

Maven + JUnit 5 test project used to verify the multi-analyzer Maven support
(see `multi-analyzer/skills/analyzer-usage/`).

## Structure

```
maven-java/
├── pom.xml                      # Maven config (JUnit 5, compiler/surefire plugins)
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
`broken` profile) compile errors so the analyzer has real issues to detect.

## Commands

```bash
mvn compile -q       # succeeds; emits deprecation warnings
mvn -Pbroken compile # fails: Broken.java compile errors
mvn test             # fails: UtilsTest.testFormatDate failure
```

## Analyzer Usage

```bash
analyzer maven "compile -q"
analyzer maven "-Pbroken compile"
analyzer maven "test"
analyzer run "mvn test"
analyzer rewrite "mvn compile"
```
