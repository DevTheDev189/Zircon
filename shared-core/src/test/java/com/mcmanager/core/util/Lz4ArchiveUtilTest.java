package com.mcmanager.core.util;

import net.jpountz.lz4.LZ4FrameOutputStream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class Lz4ArchiveUtilTest {

    @TempDir
    Path tempDir;

    @Test
    void roundTripsDirectoryStructureAndContents() throws IOException {
        Path source = tempDir.resolve("world");
        Files.createDirectories(source.resolve("region"));
        Files.writeString(source.resolve("level.dat"), "level data");
        Files.writeString(source.resolve("region").resolve("r.0.0.mca"), "chunk data");

        Path archive = tempDir.resolve("backup.tar.lz4");
        List<String> logs = new ArrayList<>();
        Lz4ArchiveUtil.compressDirectory(source, archive, null, logs);

        assertTrue(Files.isRegularFile(archive));
        assertTrue(Files.size(archive) > 0);
        // The audit log should report the archived file count and a ratio.
        assertFalse(logs.isEmpty());
        assertTrue(logs.get(0).contains("2 files"));

        Path restored = tempDir.resolve("restored");
        Lz4ArchiveUtil.extractArchive(archive, restored);

        assertEquals("level data", Files.readString(restored.resolve("level.dat")));
        assertEquals("chunk data", Files.readString(restored.resolve("region").resolve("r.0.0.mca")));
    }

    @Test
    void excludesDirectoryTreeFromArchive() throws IOException {
        Path source = tempDir.resolve("instance");
        Files.createDirectories(source.resolve("server"));
        Files.writeString(source.resolve("server").resolve("server.jar"), "jar bytes");
        Files.writeString(source.resolve("bom.json"), "{}");

        Path archive = tempDir.resolve("backup.tar.lz4");
        Lz4ArchiveUtil.compressDirectory(source, archive, source.resolve("server"), new ArrayList<>());

        Path restored = tempDir.resolve("restored");
        Lz4ArchiveUtil.extractArchive(archive, restored);

        assertTrue(Files.exists(restored.resolve("bom.json")));
        assertFalse(Files.exists(restored.resolve("server")));
    }

    @Test
    void nullExcludeDirArchivesEverything() throws IOException {
        Path source = tempDir.resolve("instance");
        Files.createDirectories(source);
        Files.writeString(source.resolve("a.txt"), "a");

        Path archive = tempDir.resolve("backup.tar.lz4");
        Lz4ArchiveUtil.compressDirectory(source, archive, null, new ArrayList<>());

        Path restored = tempDir.resolve("restored");
        Lz4ArchiveUtil.extractArchive(archive, restored);
        assertTrue(Files.exists(restored.resolve("a.txt")));
    }

    @Test
    void rejectsZipSlipEntries() throws IOException {
        // Hand-craft a malicious archive containing a "../evil.txt" entry.
        // (commons-compress refuses to write such names, so the header is
        // assembled manually to simulate an archive from an untrusted source.)
        Path archive = tempDir.resolve("evil.tar.lz4");
        writeRawTarArchiveWithSlip(archive);

        Path dest = tempDir.resolve("dest");
        IOException ex = assertThrows(IOException.class,
                () -> Lz4ArchiveUtil.extractArchive(archive, dest));
        assertTrue(ex.getMessage().contains("Zip slip"));
        // Nothing may have been written outside the destination.
        assertFalse(Files.exists(tempDir.resolve("evil.txt")));
    }

    /** Writes a minimal single-file TAR with a {@code ../evil.txt} entry. */
    private static void writeRawTarArchiveWithSlip(Path archive) throws IOException {
        byte[] header = new byte[512];
        byte[] name = "../evil.txt".getBytes(StandardCharsets.UTF_8);
        System.arraycopy(name, 0, header, 0, name.length);
        System.arraycopy("0000644\0".getBytes(StandardCharsets.US_ASCII), 0, header, 100, 8);
        System.arraycopy("0000000\0".getBytes(StandardCharsets.US_ASCII), 0, header, 108, 8);
        System.arraycopy("0000000\0".getBytes(StandardCharsets.US_ASCII), 0, header, 116, 8);
        // File size in octal (4 bytes of data).
        System.arraycopy("00000000004\0".getBytes(StandardCharsets.US_ASCII), 0, header, 124, 12);
        System.arraycopy("00000000000\0".getBytes(StandardCharsets.US_ASCII), 0, header, 136, 12);
        // Checksum field left as spaces while computing, then patched in.
        for (int i = 148; i < 156; i++) {
            header[i] = ' ';
        }
        header[156] = '0'; // typeflag: regular file
        System.arraycopy("ustar\0".getBytes(StandardCharsets.US_ASCII), 0, header, 257, 6);
        System.arraycopy("00".getBytes(StandardCharsets.US_ASCII), 0, header, 263, 2);

        int sum = 0;
        for (byte b : header) {
            sum += b & 0xFF;
        }
        byte[] checksum = (String.format("%06o", sum) + "\0 ").getBytes(StandardCharsets.US_ASCII);
        System.arraycopy(checksum, 0, header, 148, 8);

        byte[] data = new byte[512];
        System.arraycopy("boom".getBytes(StandardCharsets.UTF_8), 0, data, 0, 4);

        try (OutputStream out = Files.newOutputStream(archive);
             BufferedOutputStream buffered = new BufferedOutputStream(out);
             LZ4FrameOutputStream lz4 = new LZ4FrameOutputStream(buffered)) {
            lz4.write(header);
            lz4.write(data);
            lz4.write(new byte[1024]); // two zero blocks = end of archive
        }
    }
}
