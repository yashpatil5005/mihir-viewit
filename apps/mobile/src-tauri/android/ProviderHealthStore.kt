package ai.viewit.app

import android.util.AtomicFile
import org.json.JSONObject
import java.io.File

data class AndroidProviderHealth(
    val state: String = "registered",
    val consecutiveFailures: Int = 0,
    val totalFailures: Int = 0,
    val lastFailureKind: String? = null,
    val updatedAt: Long = System.currentTimeMillis(),
)

class ProviderHealthStore(filesDir: File) {
    private val file = AtomicFile(File(filesDir, "provider-health.json"))
    private val records = mutableMapOf<String, AndroidProviderHealth>()

    init { load() }

    @Synchronized
    fun get(providerId: String): AndroidProviderHealth = records[providerId] ?: AndroidProviderHealth()

    @Synchronized
    fun activated(providerId: String) {
        val current = get(providerId)
        records[providerId] = current.copy(state = "active", consecutiveFailures = 0, updatedAt = System.currentTimeMillis())
        persist()
    }

    @Synchronized
    fun failed(providerId: String, kind: String) {
        val current = get(providerId)
        val consecutive = current.consecutiveFailures + 1
        records[providerId] = current.copy(
            state = if (consecutive >= 3) "quarantined" else "failed",
            consecutiveFailures = consecutive,
            totalFailures = current.totalFailures + 1,
            lastFailureKind = kind,
            updatedAt = System.currentTimeMillis(),
        )
        persist()
    }

    @Synchronized
    fun removed(providerId: String) {
        records.remove(providerId)
        persist()
    }

    @Synchronized
    fun retry(providerId: String) {
        val current = get(providerId)
        records[providerId] = current.copy(
            state = "inactive",
            consecutiveFailures = 0,
            updatedAt = System.currentTimeMillis(),
        )
        persist()
    }

    @Synchronized
    fun snapshot(): Map<String, AndroidProviderHealth> = records.toMap()

    private fun load() {
        if (!file.baseFile.isFile) return
        try {
            val root = JSONObject(String(file.readFully(), Charsets.UTF_8))
            root.keys().forEach { id ->
                val value = root.getJSONObject(id)
                records[id] = AndroidProviderHealth(
                    state = value.optString("state", "registered"),
                    consecutiveFailures = value.optInt("consecutiveFailures", 0),
                    totalFailures = value.optInt("totalFailures", 0),
                    lastFailureKind = value.optString("lastFailureKind").takeIf(String::isNotEmpty),
                    updatedAt = value.optLong("updatedAt", 0),
                )
            }
        } catch (_: Exception) { records.clear() }
    }

    private fun persist() {
        val bytes = JSONObject().apply {
            records.forEach { (id, record) ->
                put(id, JSONObject().apply {
                    put("state", record.state)
                    put("consecutiveFailures", record.consecutiveFailures)
                    put("totalFailures", record.totalFailures)
                    record.lastFailureKind?.let { put("lastFailureKind", it) }
                    put("updatedAt", record.updatedAt)
                })
            }
        }.toString(2).toByteArray(Charsets.UTF_8)
        val output = file.startWrite()
        try {
            output.write(bytes)
            file.finishWrite(output)
        } catch (error: Throwable) {
            file.failWrite(output)
            throw error
        }
    }
}
