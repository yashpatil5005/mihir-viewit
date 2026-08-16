package ai.viewit.app

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MediaWorkerProtocolTest {
    @Test
    fun `worker requests require bounded identifiers paths and extensions`() {
        assertTrue(MediaWorkerProtocol.validRequest("request-1", "/tmp/input", "/tmp/output", "mkv"))
        assertFalse(MediaWorkerProtocol.validRequest("../bad", "/tmp/input", "/tmp/output", "mkv"))
        assertFalse(MediaWorkerProtocol.validRequest("request", "", "/tmp/output", "mkv"))
        assertFalse(MediaWorkerProtocol.validRequest("request", "/tmp/input", "/tmp/output", "bad/ext"))
    }
}
