package com.example;

import java.util.ArrayList;
import java.util.List;

/**
 * Main application class with intentional warnings for analyzer verification.
 */
public class App {

    private String name;
    private List<String> items;

    public App(String name) {
        this.name = name;
        this.items = new ArrayList<>();
    }

    public void addItem(String item) {
        // Intentional unchecked conversion warning (raw type)
        List rawList = new ArrayList();
        rawList.add(item);
        items.add(item);
    }

    public String getName() {
        return name;
    }

    public List<String> getItems() {
        return items;
    }

    public static void main(String[] args) {
        App app = new App("TestApp");
        app.addItem("item1");
        System.out.println("App: " + app.getName());
    }
}
