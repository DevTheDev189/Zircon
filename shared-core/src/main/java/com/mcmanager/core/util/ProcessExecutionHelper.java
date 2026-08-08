package com.mcmanager.core.util;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.File;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.concurrent.TimeUnit;

/**
 * Executes {@link ProcessBuilder} commands while streaming their combined output
 * to the logger, with an optional timeout. Used by the headless loader
 * installers (Forge / NeoForge client and server installs).
 */
public final class ProcessExecutionHelper {

    private static final Logger logger = LoggerFactory.getLogger(ProcessExecutionHelper.class);

    private ProcessExecutionHelper() {
    }

    /** Runs a command with no timeout (caller must be able to wait indefinitely). */
    public static int runProcess(List<String> command, File workingDir) throws IOException, InterruptedException {
        return runProcess(command, workingDir, null);
    }

    /**
     * Runs {@code command} in {@code workingDir}, logging each output line, and
     * returns the process exit code.
     *
     * @param timeout maximum wall-clock time for the process; {@code null} waits forever.
     * @return the exit code, or {@code -1} when the process had to be killed after {@code timeout}.
     * @throws IOException          if the process cannot be started
     * @throws InterruptedException if the calling thread is interrupted while waiting
     */
    public static int runProcess(List<String> command, File workingDir, Duration timeout)
            throws IOException, InterruptedException {
        logger.info("Executing command: {}", String.join(" ", command));

        ProcessBuilder pb = new ProcessBuilder(command);
        if (workingDir != null) {
            pb.directory(workingDir);
        }
        pb.redirectErrorStream(true);

        Process process = pb.start();

        // Drain the output on a virtual thread so a chatty process can never
        // deadlock the pipe buffer while we wait for it to exit.
        Thread.ofVirtual().name("process-output").start(() -> pump(process));

        int exitCode;
        if (timeout == null) {
            exitCode = process.waitFor();
        } else if (process.waitFor(timeout.toMillis(), TimeUnit.MILLISECONDS)) {
            exitCode = process.exitValue();
        } else {
            logger.warn("Process did not finish within {} — killing it", timeout);
            process.destroyForcibly();
            process.waitFor();
            return -1;
        }

        logger.info("Process finished with exit code: {}", exitCode);
        return exitCode;
    }

    private static void pump(Process process) {
        try (BufferedReader reader = new BufferedReader(
                new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
                logger.info("[Installer Output] {}", line);
            }
        } catch (IOException e) {
            if (process.isAlive()) {
                logger.debug("Installer output stream ended unexpectedly", e);
            }
        }
    }
}
