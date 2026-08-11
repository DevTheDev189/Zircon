package com.mcmanager.core.util;

import net.jpountz.lz4.LZ4FrameInputStream;
import net.jpountz.lz4.LZ4FrameOutputStream;
import org.apache.commons.compress.archivers.tar.TarArchiveEntry;
import org.apache.commons.compress.archivers.tar.TarArchiveInputStream;
import org.apache.commons.compress.archivers.tar.TarArchiveOutputStream;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.List;

/**
 * Packs and unpacks server instance directories as LZ4-compressed TAR archives
 * ({@code .tar.lz4}).
 *
 * <p>Compression streams each file through LZ4 frame format with a 4 MB block
 * size, which favors a good compression ratio while keeping decompression fast.
 * Extraction rejects any entry that escapes the destination directory
 * ("zip-slip" / path traversal), so archives can be restored into a live
 * instance folder safely.
 */
public final class Lz4ArchiveUtil {

    private Lz4ArchiveUtil() {
    }

    /** Running counters captured by the file-tree walk during packing. */
    private static final class PackStats {
        long fileCount;
        long uncompressedBytes;
    }

    /**
     * Packs the contents of {@code sourceDir} into a LZ4-compressed TAR archive.
     *
     * @param sourceDir    the directory whose contents are archived
     * @param targetArchive the archive file to write (e.g. {@code backup.tar.lz4})
     * @param excludeDir   optional directory tree inside {@code sourceDir} to skip
     *                     entirely; pass {@code null} to archive everything. Used
     *                     to keep pre-existing backup archives from being nested
     *                     inside a new one.
     * @param auditLogs    receives human-readable progress notes (file count,
     *                     timing, compression ratio); never {@code null}
     */
    public static void compressDirectory(Path sourceDir, Path targetArchive, Path excludeDir,
                                         List<String> auditLogs) throws IOException {
        long startTime = System.currentTimeMillis();
        PackStats stats = new PackStats();

        try (OutputStream fileOut = Files.newOutputStream(targetArchive);
             OutputStream bufferedOut = new BufferedOutputStream(fileOut);
             LZ4FrameOutputStream lz4Out = new LZ4FrameOutputStream(bufferedOut,
                     LZ4FrameOutputStream.BLOCKSIZE.SIZE_4MB);
             TarArchiveOutputStream tarOut = new TarArchiveOutputStream(lz4Out)) {

            tarOut.setLongFileMode(TarArchiveOutputStream.LONGFILE_POSIX);

            Files.walkFileTree(sourceDir, new SimpleFileVisitor<>() {
                @Override
                public FileVisitResult preVisitDirectory(Path dir, BasicFileAttributes attrs) throws IOException {
                    if (excludeDir != null && dir.startsWith(excludeDir)) {
                        return FileVisitResult.SKIP_SUBTREE;
                    }
                    return FileVisitResult.CONTINUE;
                }

                @Override
                public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) throws IOException {
                    if (excludeDir != null && file.startsWith(excludeDir)) {
                        return FileVisitResult.CONTINUE;
                    }

                    String entryName = sourceDir.relativize(file).toString().replace('\\', '/');
                    TarArchiveEntry entry = new TarArchiveEntry(file.toFile(), entryName);
                    tarOut.putArchiveEntry(entry);
                    Files.copy(file, tarOut);
                    tarOut.closeArchiveEntry();

                    stats.fileCount++;
                    stats.uncompressedBytes += attrs.size();
                    return FileVisitResult.CONTINUE;
                }
            });
            tarOut.finish();
        }

        long archiveSize = Files.size(targetArchive);
        long elapsed = System.currentTimeMillis() - startTime;
        double ratio = stats.uncompressedBytes > 0
                ? stats.uncompressedBytes / (double) archiveSize : 1.0;
        auditLogs.add(String.format(
                "Archived %d files (%d bytes) in %d ms. Compressed size: %.2f MB (ratio %.2f:1)",
                stats.fileCount, stats.uncompressedBytes, elapsed,
                archiveSize / (1024.0 * 1024.0), ratio));
    }

    /**
     * Decompresses a {@code .tar.lz4} archive into {@code destinationDir},
     * overwriting files that already exist. Entries that would escape the
     * destination directory (absolute paths or {@code ..} traversal) abort the
     * whole extraction.
     */
    public static void extractArchive(Path archiveFile, Path destinationDir) throws IOException {
        try (InputStream fileIn = Files.newInputStream(archiveFile);
             InputStream bufferedIn = new BufferedInputStream(fileIn);
             LZ4FrameInputStream lz4In = new LZ4FrameInputStream(bufferedIn);
             TarArchiveInputStream tarIn = new TarArchiveInputStream(lz4In)) {

            TarArchiveEntry entry;
            while ((entry = tarIn.getNextEntry()) != null) {
                Path targetPath = destinationDir.resolve(entry.getName()).normalize();
                if (!targetPath.startsWith(destinationDir)) {
                    throw new IOException("Zip slip attempt detected: " + entry.getName());
                }

                if (entry.isDirectory()) {
                    Files.createDirectories(targetPath);
                } else {
                    Files.createDirectories(targetPath.getParent());
                    Files.copy(tarIn, targetPath, StandardCopyOption.REPLACE_EXISTING);
                }
            }
        }
    }
}
