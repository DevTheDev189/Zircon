package com.mcmanager.server.stats;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SystemMetricsServiceTest {

    @TempDir
    Path tempDir;

    @Test
    void samplesCpuMemoryAndDisk() throws Exception {
        Path dataDir = tempDir.resolve("server-data");
        Files.createDirectories(dataDir);

        SystemMetricsService.MetricPoint point = SystemMetricsService.sample(dataDir);

        assertTrue(point.timestamp() > 0);
        assertTrue(point.systemCpuLoad() >= 0);
        assertTrue(point.processCpuLoad() >= 0);
        assertTrue(point.usedMemoryBytes() > 0);
        assertTrue(point.maxMemoryBytes() >= point.usedMemoryBytes());
        assertTrue(point.totalDiskBytes() > 0);
    }

    @Test
    void snapshotContainsCurrentAndRollingHistory() throws Exception {
        Path dataDir = tempDir.resolve("server-data");
        Files.createDirectories(dataDir);

        Map<String, Object> snapshot = SystemMetricsService.getMetricsSnapshot(dataDir);

        assertNotNull(snapshot.get("current"));
        assertNotNull(snapshot.get("history"));
        // history is capped at 60 entries
        assertTrue(((List<?>) snapshot.get("history")).size() <= 60);
        // the current point is also the newest history entry
        assertEquals(snapshot.get("current"),
                ((List<?>) snapshot.get("history")).get(((List<?>) snapshot.get("history")).size() - 1));
    }
}
