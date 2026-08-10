package example;

import com.fasterxml.jackson.databind.PropertyNamingStrategy;
import com.fasterxml.jackson.databind.annotation.JsonNaming;

import java.time.OffsetDateTime;
import java.util.List;

@JsonNaming(PropertyNamingStrategy.SnakeCaseStrategy.class)
public class TaskDto {
    public Long taskId;
    public String masterTaskConfigSlug;
    public OffsetDateTime createdOn;
    public List<Location> requestedLocations;
}
