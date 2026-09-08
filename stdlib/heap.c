// SPDX-License-Identifier: Apache-2.0

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include "stdlib.h"

#if !defined(__wasm__) && !defined(__riscv)
#include "solana_sdk.h"
#endif

#ifdef __riscv
// r55 sizes the emulator's memory from the ELF program headers, so the heap
// has to be a .bss object rather than a region picked past the end of the
// image, which would fall outside the mapped DRAM.
#define RISCV_HEAP_SIZE (64 * 1024)
static uint8_t riscv_heap[RISCV_HEAP_SIZE];

extern void __sys_revert(const void *data, uint64_t len);
#endif

/*
  There are many tradeoffs in heaps. I think for Solidity, we want:
   - small code size to reduce code length
   - not many malloc objects
   - not much memory

  So I think we should avoid fragmentation by neighbour merging. The most
  costly is walking the doubly linked list looking for free space.
*/
struct chunk
{
    struct chunk *next, *prev;
    uint32_t length;
    uint32_t allocated;
};

#ifdef __wasm__
#define HEAP_START ((struct chunk *)0x10000)

void __init_heap()
{
    struct chunk *first = HEAP_START;
    first->next = first->prev = NULL;
    first->allocated = false;
    first->length = (uint32_t)(__builtin_wasm_memory_size(0) * 0x10000 - (size_t)first - sizeof(struct chunk));
}
#elif defined(__riscv)
#define HEAP_START ((struct chunk *)riscv_heap)

void __init_heap()
{
    struct chunk *first = HEAP_START;
    first->next = first->prev = NULL;
    first->allocated = false;
    first->length = RISCV_HEAP_SIZE - sizeof(struct chunk);
}
#else
#define HEAP_START ((struct chunk *)0x300000000)

void __init_heap()
{
    struct chunk *first = HEAP_START;
    first->next = first->prev = NULL;
    first->allocated = false;
    first->length = (32 * 1024) - sizeof(struct chunk);
}
#endif

void __attribute__((noinline)) __free(void *m)
{
    struct chunk *cur = m;
    cur--;
    if (m)
    {
        cur->allocated = false;
        struct chunk *next = cur->next;
        if (next && !next->allocated)
        {
            // merge with next
            if ((cur->next = next->next) != NULL)
                cur->next->prev = cur;
            cur->length += next->length + sizeof(struct chunk);
            next = cur->next;
        }

        struct chunk *prev = cur->prev;
        if (prev && !prev->allocated)
        {
            // merge with previous
            prev->next = next;
            next->prev = prev;
            prev->length += cur->length + sizeof(struct chunk);
        }
    }
}

static void shrink_chunk(struct chunk *cur, uint32_t size)
{
    // round up to nearest 8 bytes
    size = (size + 7) & ~7;

    if (cur->length - size >= (8 + sizeof(struct chunk)))
    {
        // split and return
        void *data = (cur + 1);
        struct chunk *new = data + size;
        if ((new->next = cur->next) != NULL)
            new->next->prev = new;
        cur->next = new;
        new->prev = cur;
        new->allocated = false;
        new->length = cur->length - size - sizeof(struct chunk);
        cur->length = size;
    }
}

void *__attribute__((noinline)) __malloc(uint32_t size)
{
    struct chunk *cur = HEAP_START;

    while (cur && (cur->allocated || size > cur->length))
        cur = cur->next;

    if (cur)
    {
        shrink_chunk(cur, size);
        cur->allocated = true;
        return ++cur;
    }
    else
    {
        // go bang
#ifdef __wasm__
        __builtin_unreachable();
#elif defined(__riscv)
        __sys_revert(NULL, 0);
        __builtin_unreachable();
#else
        sol_log("out of heap memory");
        sol_panic();
#endif
        return NULL;
    }
}

void *__realloc(void *m, uint32_t size)
{
    struct chunk *cur = m;

    cur--;

    struct chunk *next = cur->next;

    if (next && !next->allocated && size <= (cur->length + next->length + sizeof(struct chunk)))
    {
        // merge with next
        cur->next = next->next;
        if (cur->next)
            cur->next->prev = cur;
        cur->length += next->length + sizeof(struct chunk);
        // resplit ..
        shrink_chunk(cur, size);
        return m;
    }
    else
    {
        // allocate new area and copy old data
        uint32_t len = cur->length;

        // if new size is smaller than the old data, only copy remaining data
        if (size < len)
            len = size;

        void *n = __malloc(size);

        // __memcpy8() copies 8 bytes at once; round up to the nearest 8 bytes
        // this is permitted because allocations are always aligned on 8 byte
        // boundaries anyway.
        __memcpy8(n, m, (len + 7) / 8);
        __free(m);
        return n;
    }
}
