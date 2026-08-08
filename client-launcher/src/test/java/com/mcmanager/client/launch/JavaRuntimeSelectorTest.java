package com.mcmanager.client.launch;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class JavaRuntimeSelectorTest {

    @Test
    void mapsPre117ToJava8() {
        assertEquals(8, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.8.9"));
        assertEquals(8, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.12.2"));
        assertEquals(8, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.16.5"));
    }

    @Test
    void maps117ToJava16() {
        assertEquals(16, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.17"));
        assertEquals(16, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.17.1"));
    }

    @Test
    void maps118Through1204ToJava17() {
        assertEquals(17, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.18.2"));
        assertEquals(17, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.19.4"));
        assertEquals(17, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.20.1"));
        assertEquals(17, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.20.4"));
    }

    @Test
    void maps1205AndNewerToJava21() {
        assertEquals(21, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.20.5"));
        assertEquals(21, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.20.6"));
        assertEquals(21, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.21"));
        assertEquals(21, JavaRuntimeSelector.getRequiredJavaMajorVersion("1.21.4"));
    }

    @Test
    void toleratesGarbageInput() {
        assertEquals(17, JavaRuntimeSelector.getRequiredJavaMajorVersion(null));
        assertEquals(17, JavaRuntimeSelector.getRequiredJavaMajorVersion("snapshot"));
    }

    @Test
    void javaExecutableIsPlatformAware() {
        String exec = JavaRuntimeSelector.javaExecutable(java.nio.file.Path.of("C:/jdk"));
        assertTrue(exec.endsWith("java.exe") || exec.endsWith("java"),
                "unexpected executable name: " + exec);
    }
}
