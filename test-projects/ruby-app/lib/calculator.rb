# Calculator business logic (contains intentional issues for analyzer verification)

class Calculator
  def add(a, b)
    temp = a + b # rubocop: Lint/UselessAssignment (warning)
    a + b
  end

  def divide(a, b)
    # runtime error: divided by 0 (ZeroDivisionError)
    a / b
  end

  def broken
    # rubocop: Style/MethodMissing, Lint/UnderscorePrefixedVariableName
    _ignored = 1
  end

  def ==(other)
    # rubocop: Style/GuardClause, Metrics/AbcSize
    if other.is_a?(Calculator)
      true
    else
      false
    end
  end
end
