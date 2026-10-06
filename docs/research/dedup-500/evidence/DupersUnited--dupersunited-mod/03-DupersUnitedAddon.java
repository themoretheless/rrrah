package wtf.dupers.dupersunited.api;

/**
 * The interface that defines the DupersUnited mod's entrypoint for addons.
 * The corresponding {@code fabric.mod.json} entrypoint key is {@code dupersunited:addon}.
 */
public interface DupersUnitedAddon {
    /**
     * Runs the addon initializer.
     * @param registry The object through which you register features to the DupersUnited mod.
     */
    void initialize(DupersUnitedRegistry registry);
}
