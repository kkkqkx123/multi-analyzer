#!/usr/bin/env ruby
# Executable script with an intentional runtime error

require_relative '../lib/calculator'

calc = Calculator.new
puts calc.add(1, 2)

# runtime error: undefined method 'nonexistent' for Calculator (NoMethodError)
calc.nonexistent
