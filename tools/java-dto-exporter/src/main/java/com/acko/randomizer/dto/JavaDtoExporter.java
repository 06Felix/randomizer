package com.acko.randomizer.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import com.github.victools.jsonschema.generator.Option;
import com.github.victools.jsonschema.generator.OptionPreset;
import com.github.victools.jsonschema.generator.SchemaGenerator;
import com.github.victools.jsonschema.generator.SchemaGeneratorConfigBuilder;
import com.github.victools.jsonschema.generator.SchemaVersion;
import com.github.victools.jsonschema.module.jackson.JacksonModule;
import com.github.victools.jsonschema.module.jackson.JacksonOption;

import java.lang.reflect.Type;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

public final class JavaDtoExporter {
    private static final int PROTOCOL_VERSION = 1;
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private JavaDtoExporter() {}

    public static void main(String[] args) {
        try {
            ExportRequest request = MAPPER.readValue(System.in, ExportRequest.class);
            ExportResponse response = export(request, Thread.currentThread().getContextClassLoader());
            MAPPER.writeValue(System.out, response);
        } catch (Exception error) {
            System.err.println(error.getMessage());
            System.exit(2);
        }
    }

    static ExportResponse export(ExportRequest request, ClassLoader classLoader) {
        if (request.protocolVersion != PROTOCOL_VERSION) {
            throw new IllegalArgumentException("unsupported exporter protocol version " + request.protocolVersion);
        }
        if (request.rootType == null || request.rootType.isBlank()) {
            throw new IllegalArgumentException("root_type must not be empty");
        }
        if (request.projectClasses == null || request.projectClasses.isBlank()) {
            throw new IllegalArgumentException("project_classes must not be empty");
        }

        Type rootType = new TypeExpressionParser(request.rootType, classLoader).parse();
        SchemaGeneratorConfigBuilder config = new SchemaGeneratorConfigBuilder(
                SchemaVersion.DRAFT_2020_12,
                OptionPreset.PLAIN_JSON
        );
        config.with(new JacksonModule(JacksonOption.RESPECT_JSONPROPERTY_REQUIRED));
        config.with(Option.DEFINITIONS_FOR_ALL_OBJECTS);
        JsonNode schema = new SchemaGenerator(config.build()).generateSchema(rootType);
        if (request.fieldPresence == FieldPresence.ALL) {
            markPropertiesRequired(schema);
        }

        ProjectClassScanner.Result scan = new ProjectClassScanner(
                Path.of(request.projectClasses),
                classLoader
        ).scan(rootType);
        if (scan.classes().isEmpty()) {
            throw new IllegalArgumentException("root type did not resolve to a class under " + request.projectClasses);
        }
        return new ExportResponse(
                PROTOCOL_VERSION,
                schema,
                scan.classes(),
                scan.inputHash(),
                List.of()
        );
    }

    private static void markPropertiesRequired(JsonNode node) {
        if (node instanceof ObjectNode object) {
            JsonNode properties = object.get("properties");
            if (properties instanceof ObjectNode propertyObject && !propertyObject.isEmpty()) {
                List<String> names = new ArrayList<>();
                propertyObject.fieldNames().forEachRemaining(names::add);
                names.sort(Comparator.naturalOrder());
                ArrayNode required = MAPPER.createArrayNode();
                names.forEach(required::add);
                object.set("required", required);
            }
            object.elements().forEachRemaining(JavaDtoExporter::markPropertiesRequired);
        } else if (node instanceof ArrayNode array) {
            array.elements().forEachRemaining(JavaDtoExporter::markPropertiesRequired);
        }
    }

    enum FieldPresence {
        ALL,
        ANNOTATED
    }

    static final class ExportRequest {
        @JsonProperty("protocol_version")
        public int protocolVersion;
        @JsonProperty("root_type")
        public String rootType;
        @JsonProperty("project_classes")
        public String projectClasses;
        @JsonProperty("field_presence")
        public FieldPresence fieldPresence = FieldPresence.ALL;
    }

    record ExportResponse(
            @JsonProperty("protocol_version") int protocolVersion,
            JsonNode schema,
            @JsonProperty("visited_classes") List<String> visitedClasses,
            @JsonProperty("input_hash") String inputHash,
            List<String> warnings
    ) {}
}
