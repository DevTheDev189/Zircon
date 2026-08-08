package com.mcmanager.server.stats;

import com.sun.management.OperatingSystemMXBean;

import java.lang.management.ManagementFactory;
import java.nio.file.FileStore;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Real-time host metrics for the admin UI "System Stats" tab: CPU (system-wide
 * and process), JVM heap usage, and free disk space on the data dir. Every call
 * to {@link #sample} appends to a rolling 60-entry history so the frontend can
 * render sparklines or track trends.
 */
public final class SystemMetricsService {

    private static final int HISTORY_LIMIT = 60;
    private static final List<MetricPoint> history = new ArrayList<>();
    private static final OperatingSystemMXBean osBean =
            (OperatingSystemMXBean) ManagementFactory.getOperatingSystemMXBean();

    /** One immutable measurement. All values are already formatted for display. */
    public record MetricPoint(
            long timestamp,
            double systemCpuLoad,
            double processCpuLoad,
            long usedMemoryBytes,
            long maxMemoryBytes,
            long totalDiskBytes,
            long freeDiskBytes
    ) {
    }

    private SystemMetricsService() {
    }

    /** Takes one measurement of the host, appends it to the history and returns it. */
    public static synchronized MetricPoint sample(Path dataDir) {
        double sysCpu = percentOf(osBean.getCpuLoad());
        double procCpu = percentOf(osBean.getProcessCpuLoad());

        Runtime runtime = Runtime.getRuntime();
        long totalMem = runtime.totalMemory();
        long freeMem = runtime.freeMemory();
        long usedMem = totalMem - freeMem;
        long maxMem = runtime.maxMemory();

        long totalDisk = 0;
        long freeDisk = 0;
        try {
            FileStore store = Files.getFileStore(dataDir);
            totalDisk = store.getTotalSpace();
            freeDisk = store.getUnallocatedSpace();
        } catch (Exception ignored) {
            // The data dir always exists (created by ConfigService); this is belt & braces.
        }

        MetricPoint point = new MetricPoint(
                System.currentTimeMillis(),
                Math.round(sysCpu * 10.0) / 10.0,
                Math.round(procCpu * 10.0) / 10.0,
                usedMem,
                maxMem,
                totalDisk,
                freeDisk);

        history.add(point);
        if (history.size() > HISTORY_LIMIT) {
            history.remove(0);
        }
        return point;
    }

    /** @return the latest measurement plus the rolling history. */
    public static synchronized Map<String, Object> getMetricsSnapshot(Path dataDir) {
        MetricPoint current = sample(dataDir);
        Map<String, Object> map = new HashMap<>();
        map.put("current", current);
        map.put("history", new ArrayList<>(history));
        return map;
    }

    /** Converts a JMX load fraction (0..1, or -1/NaN when unavailable) to a percent. */
    private static double percentOf(double load) {
        if (!Double.isFinite(load) || load < 0) {
            return 0;
        }
        return Math.min(load, 1.0) * 100.0;
    }
}
