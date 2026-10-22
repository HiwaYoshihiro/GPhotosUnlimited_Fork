package com.rev4n.unlimitedphotos;

import static org.junit.Assert.assertEquals;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Map;

import org.junit.After;
import org.junit.Before;
import org.junit.Test;

/** The feature proxy has to answer from config before it touches the binder. */
public class CustomPackageManagerTest {
    private static final String FEATURE = "com.google.android.feature.PIXEL_EXPERIENCE";

    public static final class FakePackageManager {
        int calls;

        public boolean hasSystemFeature(String name) {
            calls++;
            return false;
        }
    }

    @Before
    public void clearBefore() throws Exception {
        overrides().clear();
    }

    @After
    public void clearAfter() throws Exception {
        overrides().clear();
    }

    @SuppressWarnings("unchecked")
    private static Map<String, String> overrides() throws Exception {
        // The map is private because only receiveJson() populates it in
        // production; the test reads it directly instead of widening the API.
        Field field = EntryPoint.class.getDeclaredField("map");
        field.setAccessible(true);
        return (Map<String, String>) field.get(null);
    }

    private static Method featureMethod() throws Exception {
        return FakePackageManager.class.getDeclaredMethod("hasSystemFeature", String.class);
    }

    @Test
    public void configuredFeatureIsAnsweredWithoutCallingTheBinder() throws Throwable {
        overrides().put(FEATURE, "true");
        FakePackageManager original = new FakePackageManager();

        Object result = new CustomPackageManager(original)
                .invoke(null, featureMethod(), new Object[]{FEATURE});

        assertEquals(Boolean.TRUE, result);
        assertEquals(0, original.calls);
    }

    @Test
    public void unconfiguredFeaturePassesThrough() throws Throwable {
        FakePackageManager original = new FakePackageManager();

        Object result = new CustomPackageManager(original)
                .invoke(null, featureMethod(), new Object[]{"android.hardware.camera"});

        assertEquals(Boolean.FALSE, result);
        assertEquals(1, original.calls);
    }
}
