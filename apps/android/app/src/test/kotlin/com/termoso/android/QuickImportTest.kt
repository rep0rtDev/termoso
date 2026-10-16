package com.termoso.android

import com.termoso.android.ui.hosts.parseQuickImport
import org.junit.Assert.assertEquals
import org.junit.Test

class QuickImportTest {
    @Test
    fun looseLinesAfterConfigBlockAreImported() {
        val hosts = parseQuickImport(
            """
            Host db-01
              HostName 10.0.1.12
              User postgres

            deploy@edge.example.io:2222
            ops@bastion.example.io
            runner@10.0.2.7
            """.trimIndent(),
        )
        assertEquals(listOf("db-01", "edge.example.io", "bastion.example.io", "10.0.2.7"), hosts.map { it.label })
        assertEquals("10.0.1.12", hosts[0].address)
        assertEquals("postgres", hosts[0].username)
        assertEquals(2222, hosts[1].port)
        assertEquals("deploy", hosts[1].username)
    }

    @Test
    fun configBlocksKeepOptionsAcrossBlankLines() {
        val hosts = parseQuickImport(
            """
            Host edge
              HostName edge.example.io

              Port 2222
            Host *
              ForwardAgent yes
            """.trimIndent(),
        )
        assertEquals(1, hosts.size)
        assertEquals(2222, hosts[0].port)
    }

    @Test
    fun looseFormats() {
        val hosts = parseQuickImport("ssh root@10.0.0.2 -p 2222\nuser@[2001:db8::1]:22\n10.0.0.3")
        assertEquals(listOf(2222, 22, null), hosts.map { it.port })
        assertEquals("2001:db8::1", hosts[1].address)
        assertEquals("", hosts[2].username)
    }
}
