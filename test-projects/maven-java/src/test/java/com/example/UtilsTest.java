package com.example;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;
import java.util.Date;
import java.util.Arrays;

/**
 * Test class for Utils with one intentional failure for analyzer verification.
 */
public class UtilsTest {

    @Test
    void testFormatDate() {
        Date date = new Date(123, 0, 15); // January 15, 2023 (year is offset from 1900)
        String result = Utils.formatDate(date);
        // Intentional failure: formatDate returns "123-0-15" while the real
        // year (2023) is expected.
        assertTrue(result.startsWith("2023"), "expected 2023, got " + result);
    }

    @Test
    void testUnsafeCast() {
        String original = "test string";
        String casted = Utils.unsafeCast(original);
        assertEquals(original, casted);
    }

    @Test
    void testProcessItems() {
        // Should not throw exception
        assertDoesNotThrow(() -> {
            Utils.processItems(Arrays.asList("item1", "item2"));
        });
    }

    @Test
    void testProcessEmptyItems() {
        // Should not throw exception with empty list
        assertDoesNotThrow(() -> {
            Utils.processItems(Arrays.asList());
        });
    }
}
