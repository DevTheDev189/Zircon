# Detailed Agent Execution Plan

## Branch Strategy
The agent must execute the following Git commands prior to making changes:
```bash
git checkout -b feature/shaders-texture-packs
```
Once all phases and tests are completed, the agent will merge the feature branch back to `main`:
```bash
git checkout main
git merge feature/shaders-texture-packs
```

---

## Phase 1: Shared Core & Shader Engine Mapping

### 1.1 Create `ShaderEngineType.java`
**File**: `main/java/com/mcmanager/core/model/ShaderEngineType.java`

Define an enum mapping `ModLoaderType` to the appropriate shader engine and required dependency mods:
```java
package com.mcmanager.core.model;

import java.util.List;
import java.util.Set;

public enum ShaderEngineType {
    IRIS("Iris Shaders", "iris", "sodium", List.of("fabric", "quilt")),
    OCULUS("Oculus Shaders", "oculus", "embeddium", List.of("forge", "neoforge"));

    public static final Set<String> SHADER_MOD_PROJECT_IDS = 
        Set.of("iris", "sodium", "oculus", "embeddium", "rubidium");

    private final String displayName;
    private final String primaryProjectId;
    private final String dependencyProjectId;
    private final List<String> supportedLoaders;

    ShaderEngineType(String displayName, String primaryProjectId, String dependencyProjectId, List<String> supportedLoaders) {
        this.displayName = displayName;
        this.primaryProjectId = primaryProjectId;
        this.dependencyProjectId = dependencyProjectId;
        this.supportedLoaders = supportedLoaders;
    }

    public static ShaderEngineType forLoader(String loaderType) {
        if (loaderType == null) return IRIS;
        String normalized = loaderType.toLowerCase().trim();
        return (normalized.equals("forge") || normalized.equals("neoforge")) ? OCULUS : IRIS;
    }

    public String getDisplayName() { return displayName; }
    public String getPrimaryProjectId() { return primaryProjectId; }
    public String getDependencyProjectId() { return dependencyProjectId; }
    public List<String> getSupportedLoaders() { return supportedLoaders; }
}
```

### 1.2 Verify `ModrinthApiClient.java`
**File**: `main/java/com/mcmanager/core/api/ModrinthApiClient.java`

Ensure `searchMods` cleanly formats requests when `projectType` is `"shader"` or `"resourcepack"`:
```java
// Ensure searchMods correctly appends project_type facet when projectType is provided
if (projectType != null && !projectType.isBlank()) {
    facetGroups.add("[\"project_type:" + projectType + "\"]");
}
```

---

## Phase 2: Server-Side Storage, Services & REST APIs

### 2.1 Filter Shader Engine Mods from Standard Mod Listings
**File**: `main/java/com/mcmanager/server/service/ModManagementService.java`

Filter out shader loader project IDs from standard mod listings so they do not clutter the main Mods page:
```java
public List<ModEntry> listModsFiltered() {
    return listMods().stream()
            .filter(m -> !ShaderEngineType.SHADER_MOD_PROJECT_IDS.contains(
                    m.getId() == null ? "" : m.getId().toLowerCase()))
            .toList();
}
```

### 2.2 Create `PackManagementService.java`
**File**: `main/java/com/mcmanager/server/service/PackManagementService.java`

Manages `shaderpacks/` and `resourcepacks/` directories inside an instance:
```java
package com.mcmanager.server.service;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;

public class PackManagementService {

    private final Path instanceDir;
    private final Path shaderpacksDir;
    private final Path resourcepacksDir;

    public PackManagementService(Path instanceDir) {
        this.instanceDir = instanceDir;
        this.shaderpacksDir = instanceDir.resolve("shaderpacks");
        this.resourcepacksDir = instanceDir.resolve("resourcepacks");
        try {
            Files.createDirectories(shaderpacksDir);
            Files.createDirectories(resourcepacksDir);
        } catch (IOException e) {
            throw new IllegalStateException("Could not create pack directories", e);
        }
    }

    public List<String> listShaderpacks() throws IOException {
        return listZipFiles(shaderpacksDir);
    }

    public List<String> listResourcepacks() throws IOException {
        return listZipFiles(resourcepacksDir);
    }

    public void saveShaderpack(String filename, InputStream content) throws IOException {
        saveZip(shaderpacksDir, filename, content);
    }

    public void saveResourcepack(String filename, InputStream content) throws IOException {
        saveZip(resourcepacksDir, filename, content);
    }

    public boolean deleteShaderpack(String filename) throws IOException {
        return Files.deleteIfExists(shaderpacksDir.resolve(sanitize(filename)));
    }

    public boolean deleteResourcepack(String filename) throws IOException {
        return Files.deleteIfExists(resourcepacksDir.resolve(sanitize(filename)));
    }

    private List<String> listZipFiles(Path dir) throws IOException {
        if (!Files.isDirectory(dir)) return List.of();
        try (var stream = Files.list(dir)) {
            return stream.filter(Files::isRegularFile)
                    .map(p -> p.getFileName().toString())
                    .filter(name -> name.toLowerCase().endsWith(".zip"))
                    .toList();
        }
    }

    private void saveZip(Path dir, String filename, InputStream content) throws IOException {
        Path target = dir.resolve(sanitize(filename));
        Files.copy(content, target, StandardCopyOption.REPLACE_EXISTING);
    }

    private String sanitize(String name) {
        return name.replaceAll("[^A-Za-z0-9._\\-]", "_");
    }
}
```

### 2.3 Add Controller Methods & Routes
**File**: `main/java/com/mcmanager/server/web/controller/InstanceController.java`  
**File**: `main/java/com/mcmanager/server/web/JavalinApp.java`

Add REST handlers for shaders, resourcepacks, and shader toggle:
* `GET /api/instances/{id}/shaders` -> Returns shader engine status & list of shaderpacks.
* `POST /api/instances/{id}/shaders/toggle` -> Installs/removes Iris+Sodium or Oculus+Embeddium based on loader.
* `POST /api/instances/{id}/shaders/upload` -> Multipart upload for `.zip` shaderpack.
* `DELETE /api/instances/{id}/shaders/{filename}` -> Deletes shaderpack `.zip`.
* `GET /api/instances/{id}/resourcepacks` -> List resourcepacks.
* `POST /api/instances/{id}/resourcepacks/upload` -> Multipart upload for `.zip` resourcepack.
* `DELETE /api/instances/{id}/resourcepacks/{filename}` -> Deletes resourcepack `.zip`.

---

## Phase 3: Server Web Admin UI (`index.html`) Updates

### 3.1 Update Navigation & Filter Mods Tab
**File**: `main/resources/web/index.html`

1. **Top Navbar**: Add `"shaders"` to navigation buttons:
   ```html
   <button v-for="t in ['mods', 'shaders', 'console', 'players', 'backups', 'settings']" ...>
   ```
2. **Mods Tab Cleanup**:
   - Filter `recommendedMods` in `index.html` to exclude `sodium` and `iris`.
   - Hide shader loader mods from "Installed Mods".

### 3.2 Add Shaders & Texture Packs Tab UI
In `index.html`, add `<div v-if="activeTab === 'shaders'" ...>`:
* **Shader Engine Toggle Card**: Shows Iris Shaders / Oculus Shaders based on `selectedInstance.modLoader.type`, with an Enable/Disable toggle button.
* **Shaderpacks Section**:
  * Drag-and-Drop file dropzone (`@dragover.prevent`, `@drop.prevent="handleShaderDrop"`).
  * Modrinth Shader Search & 1-Click Install (`project_type:shader`).
  * List of installed `.zip` shaderpacks with Delete action.
* **Resource Packs Section**:
  * Drag-and-Drop file dropzone for `.zip` texture packs.
  * Modrinth Resource Pack Search & 1-Click Install (`project_type:resourcepack`).
  * List of installed `.zip` resourcepacks with Delete action.

---

## Phase 4: Client Launcher Local Options & Pack Services

### 4.1 Create `ShaderOptionsWriter.java`
**File**: `main/java/com/mcmanager/client/launch/ShaderOptionsWriter.java`

Reads/writes `optionsiris.txt` and `options.txt` in the client's game directory:
```java
package com.mcmanager.client.launch;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

public class ShaderOptionsWriter {

    private static final Logger log = LoggerFactory.getLogger(ShaderOptionsWriter.class);

    public static void setShaderOptions(Path gameDir, boolean enabled, String shaderPack) throws IOException {
        Path irisOptions = gameDir.resolve("optionsiris.txt");
        List<String> lines = Files.isRegularFile(irisOptions)
                ? new ArrayList<>(Files.readAllLines(irisOptions, StandardCharsets.UTF_8))
                : new ArrayList<>();

        setOrUpdate(lines, "enableShaders", String.valueOf(enabled));
        setOrUpdate(lines, "shaderPack", shaderPack == null ? "" : shaderPack);

        Files.write(irisOptions, lines, StandardCharsets.UTF_8);
        log.info("Updated optionsiris.txt: enableShaders={}, shaderPack={}", enabled, shaderPack);
    }

    private static void setOrUpdate(List<String> lines, String key, String value) {
        String prefix = key + "=";
        boolean found = false;
        for (int i = 0; i < lines.size(); i++) {
            if (lines.get(i).startsWith(prefix)) {
                lines.set(i, prefix + value);
                found = true;
                break;
            }
        }
        if (!found) {
            lines.add(prefix + value);
        }
    }
}
```

### 4.2 Create `ClientPackManager.java`
**File**: `main/java/com/mcmanager/client/pack/ClientPackManager.java`

Manages local shaderpacks and resourcepacks in the client launcher instance directory:
* Scans `<gameDir>/shaderpacks/` and `<gameDir>/resourcepacks/`.
* Copies dropped or picked `.zip` files into these folders.
* Downloads shaderpacks / resourcepacks directly from Modrinth CDN URLs.

---

## Phase 5: Client Launcher JavaFX UI Updates

### 5.1 Sidebar & View Switcher Updates
**File**: `main/java/com/mcmanager/client/ui/MainApp.java`

1. **Add Sidebar Button**:
   ```java
   Button navShadersPacks = new Button("🖼️  Shaders & Texture Packs");
   ```
2. **Build `shadersPacksView` Layout**:
   - **Shader Engine Card**: Displays active loader (Iris or Oculus) and a toggle switch to enable/disable shaders.
   - **Shaderpacks Card**:
     - File drop target with Drag & Drop event handlers (`setOnDragOver`, `setOnDragDropped`).
     - Modrinth Shader Search bar + Install button.
     - RadioButton / ListView of available shaderpacks allowing the user to select the active shader.
   - **Texture Packs Card**:
     - File drop target with Drag & Drop event handlers.
     - Modrinth Texture Pack Search bar + Install button.
     - ListView of available texture packs.

### 5.2 Controller Event Logic
**File**: `main/java/com/mcmanager/client/ui/controller/MainController.java`

- Add event handlers for `navShadersPacks`.
- Implement drag & drop handler:
  ```java
  dropTarget.setOnDragOver(event -> {
      if (event.getDragboard().hasFiles()) {
          event.acceptTransferModes(TransferMode.COPY);
      }
      event.consume();
  });

  dropTarget.setOnDragDropped(event -> {
      Dragboard db = event.getDragboard();
      if (db.hasFiles()) {
          for (File file : db.getFiles()) {
              if (file.getName().toLowerCase().endsWith(".zip")) {
                  clientPackManager.installLocalPack(file);
              }
          }
          event.setDropCompleted(true);
      }
      event.consume();
  });
  ```

### 5.3 Pre-Launch Options Invocation
**File**: `main/java/com/mcmanager/client/launch/MinecraftRunner.java`

Before launching Minecraft, write `optionsiris.txt`:
```java
ShaderOptionsWriter.setShaderOptions(gameDir, shadersEnabled, selectedShaderPack);
```

---

## Phase 6: Automated Testing, Verification & Git Merge

### 6.1 Unit & Integration Tests
* **`ShaderOptionsWriterTest.java`**:
  `test/java/com/mcmanager/client/launch/ShaderOptionsWriterTest.java`
  - Test creating `optionsiris.txt` from scratch.
  - Test updating existing `optionsiris.txt` entries.
* **`PackManagementServiceTest.java`**:
  `test/java/com/mcmanager/server/service/PackManagementServiceTest.java`
  - Test listing, saving, and deleting shaderpacks and resourcepacks.
* **`ShaderEngineTypeTest.java`**:
  `test/java/com/mcmanager/core/model/ShaderEngineTypeTest.java`
  - Test mapping Fabric/Quilt -> IRIS, Forge/NeoForge -> OCULUS.

### 6.2 Merge Feature Branch
Once tests pass and builds succeed:
```bash
git add .
git commit -m "feat: Add Shaders and Texture Packs management page, Iris/Oculus toggle, and options writer"
git checkout main
git merge feature/shaders-texture-packs
```
