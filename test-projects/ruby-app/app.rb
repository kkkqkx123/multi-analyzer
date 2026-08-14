# Main application entry (contains intentional issues for analyzer verification)

require_relative 'lib/calculator'

def greet(name = nil)
  # warning: Assignment Branch Condition size / Style issues (rubocop)
  greeting = "Hello, "
  if name == nil # rubocop: Style/NilComparison
    greeting = greeting + "world"
  else
    greeting = greeting + name
  end
  unused_local = 42 # rubocop: Lint/UselessAssignment
  puts greeting
  puts unused_local
end

def crash
  value = nil
  # runtime error: undefined method 'upcase' for nil (NoMethodError)
  value.upcase
end

greet("Alice")
crash
