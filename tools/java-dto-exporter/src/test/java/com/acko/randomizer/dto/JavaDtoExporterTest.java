package com.acko.randomizer.dto;

import com.fasterxml.jackson.databind.PropertyNamingStrategies;
import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.annotation.JsonNaming;
import org.junit.jupiter.api.Test;

import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class JavaDtoExporterTest {
    @Test
    void exportsJacksonNamesAndResolvedGenericData() throws Exception {
        JavaDtoExporter.ExportRequest request = new JavaDtoExporter.ExportRequest();
        request.protocolVersion = 1;
        request.rootType = Envelope.class.getName() + "<" + TaskDto.class.getName() + ">";
        request.projectClasses = Path.of(
                getClass().getProtectionDomain().getCodeSource().getLocation().toURI()
        ).toString();

        JavaDtoExporter.ExportResponse response = JavaDtoExporter.export(
                request,
                getClass().getClassLoader()
        );

        JsonNode schema = response.schema();
        assertEquals("https://json-schema.org/draft/2020-12/schema", schema.get("$schema").asText());
        assertTrue(schema.toString().contains("task_id"), schema.toPrettyString());
        assertTrue(schema.toString().contains("created_on"));
        assertFalse(schema.toString().contains("taskId"));
        assertTrue(response.visitedClasses().contains(Envelope.class.getName()));
        assertTrue(response.visitedClasses().contains(TaskDto.class.getName()));
        assertTrue(response.inputHash().startsWith("sha256:"));
    }

    @Test
    void leavesPropertiesOptionalInAnnotatedPresenceMode() throws Exception {
        JavaDtoExporter.ExportRequest request = new JavaDtoExporter.ExportRequest();
        request.protocolVersion = 1;
        request.rootType = Envelope.class.getName() + "<" + TaskDto.class.getName() + ">";
        request.projectClasses = Path.of(
                getClass().getProtectionDomain().getCodeSource().getLocation().toURI()
        ).toString();
        request.fieldPresence = JavaDtoExporter.FieldPresence.ANNOTATED;

        JsonNode schema = JavaDtoExporter.export(request, getClass().getClassLoader()).schema();

        assertFalse(schema.has("required"));
        JsonNode required = schema.get("$defs").get("TaskDto").get("required");
        assertEquals(1, required.size());
        assertEquals("task_id", required.get(0).asText());
    }

    static class Envelope<T> {
        public T data;
    }

    @JsonNaming(PropertyNamingStrategies.SnakeCaseStrategy.class)
    static class TaskDto {
        @JsonProperty(required = true)
        public Long taskId;
        public String createdOn;
    }
}
