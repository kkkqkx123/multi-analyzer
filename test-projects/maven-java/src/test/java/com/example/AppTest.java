package com.example;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.BeforeEach;
import static org.junit.jupiter.api.Assertions.*;

/**
 * Passing test class for App.
 */
public class AppTest {

    private App app;

    @BeforeEach
    void setUp() {
        app = new App("TestApp");
    }

    @Test
    void testGetName() {
        assertEquals("TestApp", app.getName());
    }

    @Test
    void testAddItem() {
        app.addItem("item1");
        assertEquals(1, app.getItems().size());
        assertTrue(app.getItems().contains("item1"));
    }

    @Test
    void testAddMultipleItems() {
        app.addItem("item1");
        app.addItem("item2");
        app.addItem("item3");
        assertEquals(3, app.getItems().size());
    }

    @Test
    void testEmptyItems() {
        assertTrue(app.getItems().isEmpty());
    }
}
