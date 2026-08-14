require_relative '../lib/calculator'

RSpec.describe Calculator do
  describe '#add' do
    it 'adds two numbers' do
      expect(Calculator.new.add(2, 3)).to eq(5)
    end
  end

  describe '#divide' do
    it 'divides two numbers' do
      expect(Calculator.new.divide(10, 2)).to eq(5)
    end

    it 'raises on division by zero' do
      expect { Calculator.new.divide(1, 0) }.to raise_error(ZeroDivisionError)
    end
  end

  describe '#broken' do
    it 'fails because of broken implementation' do
      # intentional failure: expected 42, got nil
      expect(Calculator.new.broken).to eq(42)
    end
  end
end
