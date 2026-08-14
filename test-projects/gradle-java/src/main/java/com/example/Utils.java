package com.example;

import java.util.Date;

/**
 * Utility class with intentional warnings for analyzer verification.
 */
public class Utils {

    // Intentional raw type usage
    private static java.util.List rawList = new java.util.ArrayList();

    /**
     * Format a date (intentional deprecation warning)
     */
    public static String formatDate(Date date) {
        // Using deprecated method to generate warning
        int year = date.getYear();
        int month = date.getMonth();
        int day = date.getDate();
        return year + "-" + month + "-" + day;
    }

    /**
     * Process data with unchecked cast
     */
    @SuppressWarnings("unchecked")
    public static <T> T unsafeCast(Object obj) {
        return (T) obj;
    }

    /**
     * Method with unused variable (potential warning)
     */
    public static void processItems(java.util.List<String> items) {
        String unused = "this is unused";  // Some compilers warn about this
        for (String item : items) {
            System.out.println(item);
        }
    }
}
